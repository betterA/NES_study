pub struct CPU {
    pub register_a : u8,
    pub register_x : u8, 
    pub status: u8,
    pub program_counter: u16,
    memory: [u8;0xFFFF]
}

impl CPU {
    pub fn new() -> Self {
        CPU {
            register_a: 0,
            register_x: 0, 
            status: 0,
            program_counter: 0,
            memory:[0; 0xFFFF]
        }
    } 
    // 内存相关的操作
    fn mem_read(&self, addr: u16) -> u8 {
        self.memory[addr as usize]
    }

    fn mem_write(&mut self, addr: u16, data:u8){
        self.memory[addr as usize] = data;
    }

    pub fn load_and_run(&mut self, program:Vec<u8>){
        self.load(program);
        // self.run() TODO
    }

    pub fn load(&mut self, program:Vec<u8>){
        // 将ROM LOAD 到内存 0x8000开始
        self.memory[0x8000 .. (0x8000 + program.len())].copy_from_slice(&program[..]);
        self.program_counter = 0x8000; // PC指向ROM的开始地址，然后执行程序
    }


    // 解释
    // 1. 从指令寄存器中获取下一条执行命令
    // 解码指令-> 执行指令-> 重复循环
    // program 是内存器
    pub fn interpret(&mut self, program: Vec<u8>){
        self.program_counter = 0;
        loop {
            let opscode = program[self.program_counter as usize];
            self.program_counter += 1;

            match opscode {
                0xA9 =>{ // LDA指令为两字节， 一字节是操作码本身，一字节是参数
                    let param = program[self.program_counter as usize];
                    self.program_counter += 1;
                    self.register_a = param; // 将参数LOAD 到 累加器A上
                    // 更新 处理器状态寄存器P的 bit 1 - Zero Flag and bit 7 - Negative Flag
                    self.update_zero_and_negative_flags(self.register_a);
                }
                0xE8 =>{ // INX 指令 1字节  对X寄存器加一
                    self.register_x = self.register_x.wrapping_add(1); // over_flow的捕捉
                    self.update_zero_and_negative_flags(self.register_x);

                }
                0xAA => {  // TAX 1字节 将值从 A 复制到 X，并更新状态寄存器
                    self.register_x = self.register_a;
                    self.update_zero_and_negative_flags(self.register_x);

                }
                0x00 => { // BRK 指令 
                    return;
                }
                _ => todo!()
            }

        }
        
    }
    pub fn update_zero_and_negative_flags(&mut self, register_value:u8) {
        if register_value == 0b0000_0000 {
            self.status = self.status | 0b0000_0010; // 修改ZeroFlag位为 1
        }else {
            self.status = self.status & 0b1111_1101; // 修改ZeroFlag 为  0
        }

        if register_value & 0b1000_0000 != 0 {    // 判断 reg A 是否顶位为1
            self.status = self.status | 0b1000_0000; // 为负数  修改NegativeFlag为 1
        } else {
            self.status = self.status & 0b0111_1111; // 为负数  修改NegativeFlag为 0
        }
    }
}

#[cfg(test)]
mod test{
    use super::*;

    #[test]
    fn test_0xa9_lda_immidate_load_data(){
        let mut cpu = CPU::new();
        cpu.interpret(vec![0xa9, 0x05, 0x00]);
        assert_eq!(cpu.register_a, 0x05);
        assert!(cpu.status & 0b0000_0010 == 0b0000_0000);
        assert!(cpu.status & 0b1000_0000 == 0b0000_0000);
    }

    #[test]
    fn test_0xa9_lda_zero_flag() {
        let mut cpu = CPU::new();
        cpu.interpret(vec![0xa9, 0x00, 0x00]);
        assert!(cpu.status & 0b0000_0010 == 0b0000_0010);
    }

    #[test]
    fn test_0xa9_lda_negative_flag() {
        let mut cpu = CPU::new();
        cpu.interpret(vec![0xa9, 0b1100_0000, 0x00]);
        assert!(cpu.status & 0b1000_0000 == 0b1000_0000);
    }

    #[test]
    fn test_0xaa_tax_move_a_to_x(){
        let mut cpu = CPU::new();
        cpu.register_a = 10;
        cpu.interpret(vec![0xaa, 0x00]);
        assert_eq!(cpu.register_x, 10);
    }

    #[test]
   fn test_5_ops_working_together() {
       let mut cpu = CPU::new();
       cpu.interpret(vec![0xa9, 0xc0, 0xaa, 0xe8, 0x00]);
 
       assert_eq!(cpu.register_x, 0xc1)
   }

   #[test]
   fn test_inx_overflow() {
       let mut cpu = CPU::new();
       cpu.register_x = 0xff;
       cpu.interpret(vec![0xe8, 0xe8, 0x00]);

       assert_eq!(cpu.register_x, 1)
   }

}