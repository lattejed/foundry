
use sputnik::{
    Capture, ExitReason,
    ExitSucceed, Handler, Runtime, Resolve, Machine, Memory, Opcode
};

use ethers::types::H256;

use std::{fmt::Display, borrow::Cow, rc::Rc};
/// EVM runtime.
///
/// The runtime wraps an EVM `Machine` with support of return data and context.
pub struct ForgeRuntime<'b, 'config> {
	pub inner: &'b mut Runtime<'config>,
	pub code: Rc<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct DebugStep {
	pub pc: usize,
	pub stack: Vec<H256>,
	pub memory: Memory,
	pub op: OpCode,
	pub push_bytes: Option<Vec<u8>>,
}

impl DebugStep {
	pub fn pretty_opcode(&self) -> String {
		if let Some(push_bytes) = &self.push_bytes {
			format!("{}(0x{})", self.op,  hex::encode(push_bytes))
		} else {
			self.op.to_string()
		}
	}
}

impl Display for DebugStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    	if let Some(push_bytes) = &self.push_bytes {
    		write!(f, "pc: {:?}\nop: {}(0x{})\nstack: {:#?}\nmemory: 0x{}\n\n", self.pc, self.op, hex::encode(push_bytes), self.stack, hex::encode(self.memory.data()))
    	} else {
    		write!(f, "pc: {:?}\nop: {}\nstack: {:#?}\nmemory: 0x{}\n\n", self.pc, self.op, self.stack, hex::encode(self.memory.data()))	
    	}
    }
}

impl<'b, 'config> ForgeRuntime<'b, 'config> {
	pub fn new_with_runtime(
		runtime: &'b mut Runtime<'config>,
		code: Rc<Vec<u8>>,
	) -> Self {
		Self {
			inner: runtime,
			code
		}
	}

	/// Step the runtime.
	pub fn step<'a, H: Handler>(
		&'a mut self,
		handler: &mut H,
	) -> Result<(), Capture<ExitReason, Resolve<'a, 'config, H>>> {
		self.inner.step(handler)
	}

	/// Get a reference to the machine.
	pub fn machine(&self) -> &Machine {
		&self.inner.machine()
	}

	/// Loop stepping the runtime until it stops.
	pub fn run<'a, H: Handler>(
		&'a mut self,
		handler: &mut H,
	) -> Capture<ExitReason, ()> {
		let mut done = false;
		let mut res = Capture::Exit(ExitReason::Succeed(ExitSucceed::Returned));
		while !done {
			let r = self.step(handler);
			match r {
				Ok(()) => {}
				Err(e) => { done = true;
					match e {
						Capture::Exit(s) => {res = Capture::Exit(s)},
			            Capture::Trap(_) => unreachable!("Trap is Infallible"),	
					}
				}
			}
		}
		res
	}
}

pub struct Debugger<'b, 'config> {
	pub runtime: &'b mut ForgeRuntime<'b, 'config>,
	pub steps: Vec<DebugStep>,
}

impl<'b, 'config> Debugger<'b, 'config> {
	pub fn new_with_runtime(
		runtime: &'b mut ForgeRuntime<'b, 'config>
	) -> Self {
		Self {
			runtime: runtime,
			steps: Vec::new(),
		}
	}

	pub fn debug_step<'a, H: Handler>(
		&'a mut self,
		handler: &mut H,
	) -> Result<(), Capture<ExitReason, Resolve<'a, 'config, H>>> {
		let step;
		let pc = if let Ok(pos) = self.runtime.inner.machine().position() {
			pos.clone()
		} else {
			0
		};
		let mut push_bytes = None;
		if let Some((op, stack)) = self.runtime.inner.machine().inspect() {
			let op = OpCode(op);
			if let Some(push_size) = op.push_size() {
				let push_start = pc + 1;
				let push_end = pc + 1 + push_size as usize;
				if push_end < self.runtime.code.len() {
					push_bytes = Some(self.runtime.code[push_start..push_end].to_vec());
				} else {
					panic!("PUSH{} exceeds codesize?", push_size)
				}
			}
			let mut stack = stack.data().clone();
			stack.reverse();
			step = DebugStep {
				pc,
				stack,
				memory: self.runtime.inner.machine().memory().clone(),
				op,
				push_bytes,
			}
		} else {
			let mut stack = self.runtime.inner.machine().stack().data().clone();
			stack.reverse();
			step = DebugStep {
				pc,
				stack,
				memory: self.runtime.inner.machine().memory().clone(),
				op: OpCode(Opcode::INVALID),
				push_bytes,
			}
		}
		self.steps.push(step);
		self.runtime.inner.step(handler)
	}

	/// Loop stepping the runtime until it stops.
	pub fn debug_run<'a, H: Handler>(
		&'a mut self,
		handler: &mut H,
	) -> Capture<ExitReason, ()> {
		let mut done = false;
		let mut res = Capture::Exit(ExitReason::Succeed(ExitSucceed::Returned));
		while !done {
			let r = self.debug_step(handler);
			match r {
				Ok(()) => {}
				Err(e) => { done = true;
					match e {
						Capture::Exit(s) => {res = Capture::Exit(s)},
			            Capture::Trap(_) => unreachable!("Trap is Infallible"),	
					}
				}
			}
		}
		res
	}

	pub fn print_steps(&self) {
		self.steps.iter().for_each(|step| {
			println!("{}", step);	
		});
	}
}

#[derive(Debug, Clone, Copy)]
pub struct OpCode(pub Opcode);

impl OpCode {
    pub const fn name(&self) -> &'static str {
        match self.0 {
            Opcode::STOP => "STOP",
            Opcode::ADD => "ADD",
            Opcode::MUL => "MUL",
            Opcode::SUB => "SUB",
            Opcode::DIV => "DIV",
            Opcode::SDIV => "SDIV",
            Opcode::MOD => "MOD",
            Opcode::SMOD => "SMOD",
            Opcode::ADDMOD => "ADDMOD",
            Opcode::MULMOD => "MULMOD",
            Opcode::EXP => "EXP",
            Opcode::SIGNEXTEND => "SIGNEXTEND",
            Opcode::LT => "LT",
            Opcode::GT => "GT",
            Opcode::SLT => "SLT",
            Opcode::SGT => "SGT",
            Opcode::EQ => "EQ",
            Opcode::ISZERO => "ISZERO",
            Opcode::AND => "AND",
            Opcode::OR => "OR",
            Opcode::XOR => "XOR",
            Opcode::NOT => "NOT",
            Opcode::BYTE => "BYTE",
            Opcode::SHL => "SHL",
            Opcode::SHR => "SHR",
            Opcode::SAR => "SAR",
            Opcode::SHA3 => "KECCAK256",
            Opcode::ADDRESS => "ADDRESS",
            Opcode::BALANCE => "BALANCE",
            Opcode::ORIGIN => "ORIGIN",
            Opcode::CALLER => "CALLER",
            Opcode::CALLVALUE => "CALLVALUE",
            Opcode::CALLDATALOAD => "CALLDATALOAD",
            Opcode::CALLDATASIZE => "CALLDATASIZE",
            Opcode::CALLDATACOPY => "CALLDATACOPY",
            Opcode::CODESIZE => "CODESIZE",
            Opcode::CODECOPY => "CODECOPY",
            Opcode::GASPRICE => "GASPRICE",
            Opcode::EXTCODESIZE => "EXTCODESIZE",
            Opcode::EXTCODECOPY => "EXTCODECOPY",
            Opcode::RETURNDATASIZE => "RETURNDATASIZE",
            Opcode::RETURNDATACOPY => "RETURNDATACOPY",
            Opcode::EXTCODEHASH => "EXTCODEHASH",
            Opcode::BLOCKHASH => "BLOCKHASH",
            Opcode::COINBASE => "COINBASE",
            Opcode::TIMESTAMP => "TIMESTAMP",
            Opcode::NUMBER => "NUMBER",
            Opcode::DIFFICULTY => "DIFFICULTY",
            Opcode::GASLIMIT => "GASLIMIT",
            Opcode::CHAINID => "CHAINID",
            Opcode::SELFBALANCE => "SELFBALANCE",
            Opcode::BASEFEE => "BASEFEE",
            Opcode::POP => "POP",
            Opcode::MLOAD => "MLOAD",
            Opcode::MSTORE => "MSTORE",
            Opcode::MSTORE8 => "MSTORE8",
            Opcode::SLOAD => "SLOAD",
            Opcode::SSTORE => "SSTORE",
            Opcode::JUMP => "JUMP",
            Opcode::JUMPI => "JUMPI",
            Opcode::PC => "PC",
            Opcode::MSIZE => "MSIZE",
            Opcode::GAS => "GAS",
            Opcode::JUMPDEST => "JUMPDEST",
            Opcode::PUSH1 => "PUSH1",
            Opcode::PUSH2 => "PUSH2",
            Opcode::PUSH3 => "PUSH3",
            Opcode::PUSH4 => "PUSH4",
            Opcode::PUSH5 => "PUSH5",
            Opcode::PUSH6 => "PUSH6",
            Opcode::PUSH7 => "PUSH7",
            Opcode::PUSH8 => "PUSH8",
            Opcode::PUSH9 => "PUSH9",
            Opcode::PUSH10 => "PUSH10",
            Opcode::PUSH11 => "PUSH11",
            Opcode::PUSH12 => "PUSH12",
            Opcode::PUSH13 => "PUSH13",
            Opcode::PUSH14 => "PUSH14",
            Opcode::PUSH15 => "PUSH15",
            Opcode::PUSH16 => "PUSH16",
            Opcode::PUSH17 => "PUSH17",
            Opcode::PUSH18 => "PUSH18",
            Opcode::PUSH19 => "PUSH19",
            Opcode::PUSH20 => "PUSH20",
            Opcode::PUSH21 => "PUSH21",
            Opcode::PUSH22 => "PUSH22",
            Opcode::PUSH23 => "PUSH23",
            Opcode::PUSH24 => "PUSH24",
            Opcode::PUSH25 => "PUSH25",
            Opcode::PUSH26 => "PUSH26",
            Opcode::PUSH27 => "PUSH27",
            Opcode::PUSH28 => "PUSH28",
            Opcode::PUSH29 => "PUSH29",
            Opcode::PUSH30 => "PUSH30",
            Opcode::PUSH31 => "PUSH31",
            Opcode::PUSH32 => "PUSH32",
            Opcode::DUP1 => "DUP1",
            Opcode::DUP2 => "DUP2",
            Opcode::DUP3 => "DUP3",
            Opcode::DUP4 => "DUP4",
            Opcode::DUP5 => "DUP5",
            Opcode::DUP6 => "DUP6",
            Opcode::DUP7 => "DUP7",
            Opcode::DUP8 => "DUP8",
            Opcode::DUP9 => "DUP9",
            Opcode::DUP10 => "DUP10",
            Opcode::DUP11 => "DUP11",
            Opcode::DUP12 => "DUP12",
            Opcode::DUP13 => "DUP13",
            Opcode::DUP14 => "DUP14",
            Opcode::DUP15 => "DUP15",
            Opcode::DUP16 => "DUP16",
            Opcode::SWAP1 => "SWAP1",
            Opcode::SWAP2 => "SWAP2",
            Opcode::SWAP3 => "SWAP3",
            Opcode::SWAP4 => "SWAP4",
            Opcode::SWAP5 => "SWAP5",
            Opcode::SWAP6 => "SWAP6",
            Opcode::SWAP7 => "SWAP7",
            Opcode::SWAP8 => "SWAP8",
            Opcode::SWAP9 => "SWAP9",
            Opcode::SWAP10 => "SWAP10",
            Opcode::SWAP11 => "SWAP11",
            Opcode::SWAP12 => "SWAP12",
            Opcode::SWAP13 => "SWAP13",
            Opcode::SWAP14 => "SWAP14",
            Opcode::SWAP15 => "SWAP15",
            Opcode::SWAP16 => "SWAP16",
            Opcode::LOG0 => "LOG0",
            Opcode::LOG1 => "LOG1",
            Opcode::LOG2 => "LOG2",
            Opcode::LOG3 => "LOG3",
            Opcode::LOG4 => "LOG4",
            Opcode::CREATE => "CREATE",
            Opcode::CALL => "CALL",
            Opcode::CALLCODE => "CALLCODE",
            Opcode::RETURN => "RETURN",
            Opcode::DELEGATECALL => "DELEGATECALL",
            Opcode::CREATE2 => "CREATE2",
            Opcode::STATICCALL => "STATICCALL",
            Opcode::REVERT => "REVERT",
            Opcode::INVALID => "INVALID",
            Opcode::SUICIDE => "SELFDESTRUCT",
            _ => "UNDEFINED",
        }
    }

    pub fn push_size(self) -> Option<u8> {
        self.0.is_push()
    }
}

impl Display for OpCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = self.name();

        let n = if name == "UNDEFINED" {
            Cow::Owned(format!("UNDEFINED(0x{:02x})", self.0.0))
        } else {
            Cow::Borrowed(name)
        };
        write!(f, "{}", n)
    }
}

