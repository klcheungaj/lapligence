// llg-test-fixture: tests/fixtures/sim/process_control/bound_updates_exit.sv
// IEEE 1800-2009 24.7 and decision S09-D8: $exit ends the processes of its
// program and with them the timed triggers they have pending, also those of a
// process that has finished. The other program's triggers fire.
`timescale 1ns/1ns
program leaves;
  initial begin
    ->> #5 tb.e1;
    ->> @(posedge tb.s) tb.e2;
  end
  initial begin
    ->> #5 tb.e3;
    #50;
  end
  initial #2 if ($test$plusargs("exit")) $exit;
endprogram

program stays;
  initial begin
    ->> #5 tb.e4;
    #100 tb.done = 1;
  end
endprogram

module tb;
  event e1, e2, e3, e4;
  int n1, n2, n3, n4, done;
  logic s = 0;
  always @(e1) n1++;
  always @(e2) n2++;
  always @(e3) n3++;
  always @(e4) n4++;
  initial #8 s = 1;
  leaves a();
  stays b();
  final $display("n1=%0d n2=%0d n3=%0d n4=%0d done=%0d", n1, n2, n3, n4, done);
endmodule
