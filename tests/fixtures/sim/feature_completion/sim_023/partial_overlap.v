// SIM-023 A01: constant net selects, overlapping forces and partial releases.
module tb;
  reg [7:0] d, e;
  wire [7:0] n;
  wire [7:0] n2;
  wire [7:0] r;
  assign n = d;
  assign n2 = ~d;
  assign r[7:4] = e[7:4];
  assign r[3:0] = e[3:0];
  initial begin
    d = 8'h00; e = 8'h00;
    force n[7:4] = 4'hf;
    force n[1] = 1'b1;
    #1 $display("1 n=%b", n);
    force n[5:2] = 4'b0000;
    #1 $display("2 n=%b", n);
    release n[3:2];
    #1 $display("3 n=%b", n);
    d = 8'hff;
    #1 $display("4 n=%b", n);
    release n;
    #1 $display("5 n=%b", n);
    force {n[0], n2[7:6]} = 3'b101;
    #1 $display("6 n=%b n2=%b", n, n2);
    d = 8'h0f;
    #1 $display("7 n=%b n2=%b", n, n2);
    release {n[0], n2[7:6]};
    #1 $display("8 n=%b n2=%b", n, n2);
    force n[3 +: 2] = 2'b10;
    force n[7 -: 2] = 2'b11;
    #1 $display("9 n=%b", n);
    release n[3 +: 2];
    #1 $display("10 n=%b", n);
    release n[7:6];
    force r[5:2] = 4'b1010;
    e = 8'h0f;
    #1 $display("11 r=%b n=%b", r, n);
    e = 8'hf0;
    #1 $display("12 r=%b", r);
    release r[4];
    #1 $display("13 r=%b", r);
    release r;
    #1 $display("14 r=%b", r);
    $finish;
  end
endmodule
