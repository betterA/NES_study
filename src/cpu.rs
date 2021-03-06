/*
NES 6502的内存空间
[0x0000, 0x1FFF ]  CPU RAM
[0x2000, 0x401F ]  IO/Registers
[0x4020, 0x5FFF ]  特殊的扩展空间
[0x6000, 0x7FFF ]  磁带上的RAM, 用于检测游戏状态或存储
[0x8000, 0xFFFF ]  游戏ROM映射空间
*/
use std::collections::HashMap;
use crate::opscodes;

/*实际的嵌入式编程时，可能需要应对非常多的寄存器和每个寄存器bits的的映设关系！
一旦出错不好排查！所以大家就想如果可以将位操作和rust的类型系统绑定起来，
抽象封装成一个个类型和有意义的名字， 将映设关系固化下来，并且自动完成转化！
从而增强语义和表达力，这样会很好用且容易排查错误！
*/
bitflags! {
     ///
    ///  7 6 5 4 3 2 1 0
    ///  N V _ B D I Z C
    ///  | |   | | | | +--- Carry Flag
    ///  | |   | | | +----- Zero Flag
    ///  | |   | | +------- Interrupt Disable
    ///  | |   | +--------- Decimal Mode (not used on NES)
    ///  | |   +----------- Break Command
    ///  | +--------------- Overflow Flag
    ///  +----------------- Negative Flag
    ///
    pub struct CpuFlags: u8 {
        const CARRY             = 0b00000001;
        const ZERO              = 0b00000010;
        const INTERRUPT_DISABLE = 0b00000100;
        const DECIMAL_MODE      = 0b00001000;
        const BREAK             = 0b00010000;
        const BREAK2            = 0b00100000;
        const OVERFLOW          = 0b01000000;
        const NEGATIV           = 0b10000000;
    }
}

const STACK: u16 = 0x0100;
const STACK_RESET: u8 = 0xfd;

pub struct CPU {
    pub register_a: u8,
    pub register_x: u8,
    pub register_y: u8,
    pub status: CpuFlags,
    pub program_counter: u16,
    pub stack_pointer: u8, // 栈
    memory: [u8; 0xFFFF],
}

#[derive(Debug)]
#[allow(non_camel_case_types)]
pub enum AddressingMode {
    // 声明不同的寻找方式，不同的寻址方式支持的内存也有所不同
    Immediate,
    ZeroPage,
    ZeroPage_X,
    ZeroPage_Y,
    Absolute,
    Absolute_X,
    Absolute_Y,
    Indirect_X,
    Indirect_Y,
    NoneAddressing,
}

trait Mem {  // 内存接口的定义
    fn mem_read(&self, addr: u16) -> u8;

    fn mem_write(&mut self, addr: u16, data: u8);
    /*
    存储地址需要2个字节，6502使用的是小端寻址，
    这意味着地址的 8 个最低有效位将存储在 8 个最高有效位之前。
    真实地址：0x8000
    大端封装的地址：80 00
    小端封装的地址：00 80
    */
    fn mem_read_u16(&self, pos: u16) -> u16 {
        // LDA $8000  <=>  ad 00 80  pos传进来的是 00的内存地址
        let lo = self.mem_read(pos) as u16;
        let hi = self.mem_read(pos + 1) as u16;
        (hi << 8) | (lo as u16)
    }

    fn mem_write_u16(&mut self, pos: u16, data: u16) {
        // 写2字节的数据，也要小端封装  0x8000 => 00 80
        let hi = (data >> 8) as u8;
        let lo = (data & 0xff) as u8;
        self.mem_write(pos, lo);   // 00
        self.mem_write(pos + 1, hi); // 80
    }
}

impl Mem for CPU {
    fn mem_read(&self, addr: u16) -> u8 {
        self.memory[addr as usize]
    }

    fn mem_write(&mut self, addr: u16, data: u8) {
        self.memory[addr as usize] = data;
    }
}


impl CPU {
    pub fn new() -> Self {
        CPU {
            register_a: 0,
            register_x: 0,
            register_y: 0,
            stack_pointer: STACK_RESET,
            status: CpuFlags::from_bits_truncate(0b100100),
            program_counter: 0,
            memory: [0; 0xFFFF],
        }
    }

    /*
    插入新卡会触发 "Reset interrupt"的特殊信号
    该信号指示CPU:
    重置状态（寄存器和标志）
    将program_counter设置为存储在 0xFFFC 的 16 位地址
    */
    pub fn reset(&mut self) {
        self.register_a = 0;
        self.register_x = 0;
        self.status = CpuFlags::from_bits_truncate(0b100100);
        self.program_counter = self.mem_read_u16(0xFFFC);
    }

    pub fn load_and_run(&mut self, program: Vec<u8>) {
        self.load(program);
        self.reset();
        self.run();
    }

    pub fn load(&mut self, program: Vec<u8>) {
        // 将ROM LOAD 到内存 0x8000开始
        self.memory[0x8000..(0x8000 + program.len())].copy_from_slice(&program[..]);
        // self.program_counter = 0x8000; // PC指向ROM的开始地址，然后执行程序
        self.mem_write_u16(0xFFFC, 0x8000);
    }


    // 命令
    // LOAD Y
    fn ldy (&mut self, mode:&AddressingMode){
        let addr = self.get_operand_address(mode);
        let data = self.mem_read(addr);
        self.register_y = data;
        self.update_zero_and_negative_flags(self.register_y);
    }
    // LOAD A
    fn lda(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode); // 寻址方式的修改
        let value = self.mem_read(addr);
        self.register_a = value; // 将参数LOAD 到 累加器A上
                                 // 更新 处理器状态寄存器P的 bit 1 - Zero Flag and bit 7 - Negative Flag
        self.update_zero_and_negative_flags(self.register_a);
    }
    // LOAD X
    fn ldx(&mut self, mode: &AddressingMode){
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        self.register_x = value;
        self.update_zero_and_negative_flags(self.register_x);
    }


    // STORE A
    fn sta(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        self.mem_write(addr, self.register_a);
    }
    // STORE X
    fn stx(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        self.mem_write(addr, self.register_x);
    }
    // STORE Y
    fn sty(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        self.mem_write(addr, self.register_y);
    }



    fn adc(&mut self, mode: &AddressingMode){
        let addr = self.get_operand_address(mode); // 这个值加到 a 上面 // a=>只能 u8
        let data = self.mem_read(addr);
        self.register_a = self.register_a.wrapping_add(data);
        self.update_zero_and_negative_flags(self.register_a);
        // 是否 carry

    }

    fn inx(&mut self) {
        // INX 指令 1字节  对X寄存器加一
        self.register_x = self.register_x.wrapping_add(1); // over_flow的捕捉
        self.update_zero_and_negative_flags(self.register_x);
    }

    fn tax(&mut self) {
        // TAX 1字节 将值从 A 复制到 X，并更新状态寄存器
        self.register_x = self.register_a;
        self.update_zero_and_negative_flags(self.register_x);
    }
    // 解释
    // 1. 从指令寄存器中获取下一条执行命令
    // 解码指令-> 执行指令-> 重复循环
    // program 是内存器
    pub fn run(&mut self) {
        // 运行ROM中的代码, 这是通过内存的方式读取
        let ref opcodes:HashMap<u8, &'static opscodes::OpCode> = *opscodes::OPCODES_MAP;
        loop {
            let code = self.mem_read(self.program_counter);
            self.program_counter += 1;
            let program_counter_state = self.program_counter;

            let opcode = opcodes.get(&code).expect(&format!("OpCode {:x} is not recognized", code));

            match code {
                0xa9 | 0xa5 | 0xb5 | 0xad | 0xbd | 0xb9 | 0xa1 | 0xb1 => {
                    self.lda(&opcode.mode);
                } 
                /* STA */
                0x85 | 0x95 | 0x8d | 0x9d | 0x99 | 0x81 | 0x91 => {
                    self.sta(&opcode.mode);
                }
                0x69 => self.adc(&opcode.mode), 
                0xE8 => self.inx(),
                0xAA => self.tax(),
                0x00 => return, // BRK 命令
                _ => todo!(),
            }

            if program_counter_state == self.program_counter {
                self.program_counter += (opcode.len -1) as u16;
            }
        }
    }

    pub fn update_zero_and_negative_flags(&mut self, register_value: u8) {
        if register_value == 0b0000_0000 {
            self.status.insert(CpuFlags::ZERO); // 修改ZeroFlag位为 1
        } else {
            self.status.remove(CpuFlags::ZERO); // 修改ZeroFlag 为  0
        }

        if register_value & 0b1000_0000 != 0 {
            // 判断 reg A 是否顶位为1
            self.status.insert(CpuFlags::NEGATIV); // 为负数  修改NegativeFlag为 1
        } else {
            self.status.remove(CpuFlags::NEGATIV); // 为负数  修改NegativeFlag为 0
        }
    }

    fn get_operand_address(&self, mode: &AddressingMode) -> u16 {
        match mode {
            AddressingMode::Immediate => self.program_counter,
            AddressingMode::ZeroPage => self.mem_read(self.program_counter) as u16,
            AddressingMode::Absolute => self.mem_read_u16(self.program_counter),

            AddressingMode::ZeroPage_X => {
                let pos = self.mem_read(self.program_counter);
                let addr = pos.wrapping_add(self.register_x) as u16;
                addr
            }
            AddressingMode::ZeroPage_Y => {
                let pos = self.mem_read(self.program_counter);
                let addr = pos.wrapping_add(self.register_y) as u16;
                addr
            }

            AddressingMode::Absolute_X => {
                let base = self.mem_read_u16(self.program_counter);
                let addr = base.wrapping_add(self.register_x as u16);
                addr
            }
            AddressingMode::Absolute_Y => {
                let base = self.mem_read_u16(self.program_counter);
                let addr = base.wrapping_add(self.register_y as u16);
                addr
            }

            AddressingMode::Indirect_X => {
                let base = self.mem_read(self.program_counter);

                let ptr: u8 = (base as u8).wrapping_add(self.register_x);
                let lo = self.mem_read(ptr as u16);
                let hi = self.mem_read(ptr.wrapping_add(1) as u16);
                (hi as u16) << 8 | (lo as u16)
            }
            AddressingMode::Indirect_Y => {
                let base = self.mem_read(self.program_counter);

                let lo = self.mem_read(base as u16);
                let hi = self.mem_read((base as u8).wrapping_add(1) as u16);
                let deref_base = (hi as u16) << 8 | (lo as u16);
                let deref = deref_base.wrapping_add(self.register_y as u16);
                deref
            }

            AddressingMode::NoneAddressing => {
                panic!("mode {:?} is not supported", mode);
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_0xa9_lda_immidate_load_data() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xa9, 0x05, 0x00]);
        assert_eq!(cpu.register_a, 0x05);
        assert!(cpu.status.bits() & 0b0000_0010 == 0b0000_0000);
        assert!(cpu.status.bits() & 0b1000_0000 == 0b0000_0000);
    }

    #[test]
    fn test_0xa9_lda_zero_flag() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xa9, 0x00, 0x00]);
        assert!(cpu.status.bits() & 0b0000_0010 == 0b0000_0010);
    }

    #[test]
    fn test_0xa9_lda_negative_flag() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xa9, 0b1100_0000, 0x00]);
        assert!(cpu.status.bits() & 0b1000_0000 == 0b1000_0000);
    }

    #[test]
    fn test_0xaa_tax_move_a_to_x() {
        let mut cpu = CPU::new();
        // cpu.register_a = 10;  LDA #$10 TAX
        cpu.load_and_run(vec![0xa9, 0x0a, 0xaa, 0x00]);
        assert_eq!(cpu.register_x, 10);
    }

    #[test]
    fn test_inx_overflow() {
        let mut cpu = CPU::new();
        // cpu.register_x = 0xff;
        cpu.load_and_run(vec![0xa9, 0xff, 0xe8, 0x00]);

        assert_eq!(cpu.register_x, 1)
    }

    #[test]
    fn test_5_ops_working_together() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xa9, 0xc0, 0xaa, 0xe8, 0x00]);

        assert_eq!(cpu.register_x, 0xc1)
    }

    #[test]
    fn test_lda_from_memory() {
        let mut cpu = CPU::new();
        cpu.mem_write(0x10, 0x55);
        cpu.load_and_run(vec![0xa5, 0x10, 0x00]);
        assert_eq!(cpu.register_a, 0x55);
    }

    // #[test]
    // fn test_adc_from_data() {
    //     let mut cpu = CPU::new();
    //     cpu.load_and_run(vec![0xa9, 0xff, 0xaa, 0xe8, 0x69, 0xc4, 0x00]);
    //     assert_eq!(cpu.register_a, 0xc3);
    //     assert_eq!(cpu.status.bits(), 0b1000_0001);
    // }


}
