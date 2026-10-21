// SystemVerilog non-ANSI output variable port declarations with initializers.
module child(o, q, r);
  output logic o = 1'b1;
  output int q = 9;
  output var logic [3:0] r = 4'h6;
endmodule

module tb;
  wire po;
  int pq;
  wire [3:0] pr;
  child u(.o(po), .q(pq), .r(pr));
  initial #1 begin
    $display("po=%b pq=%0d pr=%h", po, pq, pr);
    $finish;
  end
endmodule
