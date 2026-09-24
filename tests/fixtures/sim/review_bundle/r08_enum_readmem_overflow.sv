// llg-test-fixture: tests/fixtures/sim/review_bundle/r08_enum_readmem_overflow.sv
// IEEE 1800-2009 21.4.2: out-of-range numeric enum data must stop the read.
module tb;
  typedef enum logic [1:0] { ZERO=2'd0, ONE=2'd1 } enum_t;
  enum_t memory[0:1];
  initial begin
    memory[0]=ONE; memory[1]=ZERO;
    $readmemh("enum_overflow.hex",memory);
    $display("AFTER_ENUM_LOAD %0d %0d",memory[0],memory[1]);
    $finish;
  end
endmodule
