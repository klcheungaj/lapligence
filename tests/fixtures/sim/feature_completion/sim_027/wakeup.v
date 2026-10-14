// SIM-027 A02: a load stores each word as the task runs; every changed word
// is an update event (IEEE 1364-2001 5.3, 17.2.8; IEEE 1800-2009 4.3, 21.4),
// so each waiter on that word runs once in the same time slot and reads the
// loaded value. A word loaded with its current value is no event, and words
// outside the range are untouched. The order of the woken processes and of
// the loading process is not fixed (IEEE 1364-2001 5.4.2; IEEE 1800-2009
// 4.7): the test accepts every order in which `caller` precedes `tick`
// (statement order, 5.4.1/4.6) and `done` comes last.
module tb;
  reg [7:0] mem [0:3];
  reg tick;
  reg armed;
  always @(mem[0]) if (armed) $display("reader0=%h", mem[0]);
  always @(mem[0]) if (armed) $display("reader0b=%h", mem[0]);
  always @(mem[1]) if (armed) $display("reader1=%h", mem[1]);
  always @(mem[2]) if (armed) $display("reader2=%h", mem[2]);
  always @(mem[3]) if (armed) $display("reader3=%h", mem[3]);
  always @(tick) if (armed) $display("tick=%b", tick);
  initial begin
    armed = 0;
    tick = 0;
    mem[0] = 8'h00;
    mem[1] = 8'h22;
    mem[2] = 8'h00;
    mem[3] = 8'h00;
    #1;
    armed = 1;
    $readmemh("wake.hex", mem, 0, 2);
    $display("caller=%h,%h,%h,%h", mem[0], mem[1], mem[2], mem[3]);
    tick = 1;
    #1;
    $display("done");
    $finish;
  end
endmodule
