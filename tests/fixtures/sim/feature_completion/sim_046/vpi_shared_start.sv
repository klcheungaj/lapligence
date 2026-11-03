// Statement coverage of shared bodies started late through the VPI.
module leaf(input logic go);
  int n = 0;
  always @(posedge go) begin
    n++;
    if (n > 2) n = 0;
  end
endmodule

module tb;
  logic go = 1'b0;
  leaf u0(go);
  leaf u1(go);
  leaf u2(go);
  leaf u3(go);
  leaf u4(go);
  task automatic pulse;
    #1 go = 1'b1;
    #1 go = 1'b0;
  endtask
  initial begin
    #0 $cov_phase(0);
    pulse;
    pulse;
    pulse;
    $cov_phase(1);
    pulse;
    pulse;
    $cov_phase(2);
    pulse;
    $cov_phase(3);
    $finish;
  end
endmodule
