module tb;
  reg [7:0] mem [0:2];
  reg tick;
  reg armed;
  always @(mem[0]) if (armed) $display("Q02.wakeup mem0_A=%b", mem[0]);
  always @(mem[0]) if (armed) $display("Q02.wakeup mem0_B=%b", mem[0]);
  always @(mem[1]) if (armed) $display("Q02.wakeup mem1=%b", mem[1]);
  always @(mem[2]) if (armed) $display("Q02.wakeup mem2=%b", mem[2]);
  always @(tick) if (armed) $display("Q02.wakeup tick=%b", tick);
  initial begin
    armed = 0; tick = 0;
    mem[0] = 0; mem[1] = 0; mem[2] = 0;
    #1; armed = 1;
    $readmemh("q02_wakeup.mem", mem, 0, 2);
    tick = 1;
    #1; $display("Q02.wakeup done=%b,%b,%b", mem[0], mem[1], mem[2]);
    $finish;
  end
endmodule
