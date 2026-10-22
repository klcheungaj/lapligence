// Bit-selects and constant part-selects as port references, implicit and
// explicit, of output variables, output nets and input nets, including a
// parameterized select.
module sv(.p(a[1:0]), .q(a[3:2]));
  output logic [3:0] a;
  initial begin a = 4'b0110; #2 a = 4'b1001; end
endmodule

module sn(.p(n[1:0]), .q(n[3:2]));
  output [3:0] n;
  assign n = 4'b1100;
endmodule

module si(a[3], a[2:0], .r(b[0]));
  input [3:0] a;
  input [1:0] b;
endmodule

module sp(.p(r[1:0]));
  output logic [3:0] r;
  initial begin r = 4'b1110; #2 r[1:0] = 2'b01; #2 r[3] = 0; end
endmodule

module spar #(parameter W = 4, parameter L = 1) (.p(a[L+1:L]));
  output logic [W-1:0] a;
  initial a = 8'b10110110;
endmodule

module tb;
  wire [1:0] w1, w2;
  logic [1:0] v1, v2;
  wire [1:0] x1, x2;
  logic [2:0] lo;
  logic [1:0] pv;
  wire [3:0] pw;
  wire [1:0] q1, q2;
  sv c1(.p(w1), .q(w2));
  sv c2(v1, v2);
  sn c3(.q(x2), .p(x1));
  si c4(1'b1, lo, 1'b1);
  sp c5(.p(pv));
  sp c6(.p(pw[2:1]));
  assign pw[0] = 1'b1;
  spar #(.W(8), .L(2)) c7(.p(q1));
  spar c8(.p(q2));
  initial lo = 3'b010;
  initial begin
    #1 $display("1 w=%b%b v=%b%b x=%b%b pv=%b pw=%b q1=%b q2=%b",
                w2, w1, v2, v1, x2, x1, pv, pw, q1, q2);
    $display("1 si a=%b b=%b", c4.a, c4.b);
    #2 $display("3 w=%b%b v=%b%b pv=%b pw=%b", w2, w1, v2, v1, pv, pw);
    #2 $display("5 pv=%b pw=%b r=%b", pv, pw, c5.r);
    $finish;
  end
endmodule
