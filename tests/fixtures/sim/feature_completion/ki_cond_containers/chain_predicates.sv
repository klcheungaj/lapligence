// Predicates of every type, over conditionals of one and of five levels
// (IEEE 1800-2009 11.4.11, 12.4).
module tb;
  typedef struct { int a; string t; } rec_t;
  typedef int q_t[$];
  rec_t r0 = '{0, "s"}, r1 = '{1, "s"}, r2 = '{2, "s"}, r3 = '{3, "s"}, r4 = '{4, "t"}, r5 = '{5, "t"}, ry;
  q_t q0 = '{0, 9}, q1 = '{1, 9}, q2 = '{2, 9}, q3 = '{3, 9}, q4 = '{4, 8}, q5 = '{5, 8}, qy;
  parameter P = 1;
  parameter logic X = 1'bx;
  logic [3:0] w;
  logic signed [64:0] g;
  real r;
  string str = "one";
  task automatic show(input string what, input rec_t v, input q_t q);
    $display("%s: %0d <%s> %0d %0d", what, v.a, v.t, q[0], q[1]);
  endtask
  task automatic each();
    ry = w ? r0 : r1; qy = w ? q0 : q1; show("vector", ry, qy);
    ry = g ? r0 : r1; qy = g ? q0 : q1; show("signed", ry, qy);
    ry = r ? r0 : r1; qy = r ? q0 : q1; show("real", ry, qy);
    ry = P ? r0 : r1; qy = !P ? q0 : q1; show("parameter", ry, qy);
    ry = X ? r0 : r1; qy = X ? q0 : q1; show("unknown", ry, qy);
    ry = '1 ? r0 : r1; qy = '0 ? q0 : q1; show("fill", ry, qy);
    ry = 'x ? r0 : r1; qy = 'z ? q0 : q1; show("fill x", ry, qy);
    ry = -3 ? r0 : r1; qy = 0.25 ? q0 : q1; show("literal", ry, qy);
    ry = str == "one" ? r0 : r1; qy = str > "p" ? q0 : q1; show("string", ry, qy);
    ry = w ? r0 : r ? r1 : g ? r2 : P ? r3 : X ? r4 : r5;
    qy = w ? q0 : r ? q1 : g ? q2 : !P ? q3 : X ? q4 : q5; show("chain", ry, qy);
    ry = r ? r0 : 'x ? r1 : -g ? r2 : w[3] ? r3 : '0 ? r4 : r5;
    qy = r ? q0 : 'x ? q1 : -g ? q2 : w[3] ? q3 : '0 ? q4 : q5; show("mixed", ry, qy);
    ry = '1 ? (g ? (r ? (w ? (X ? r0 : r1) : r2) : r3) : r4) : r5;
    qy = P ? (g ? (r ? (w ? (X ? q0 : q1) : q2) : q3) : q4) : q5; show("first", ry, qy);
  endtask
  initial begin
    w = 0; g = 0; r = 0; each();
    w = 4'b1000; g = 65'h1_0000_0000_0000_0000; r = 0.5; each();
    w = 4'b0x00; g = -1; r = -2; each();
    w = 4'b1x00; g = 65'sbz; r = 0; each();
    w = 4'b00z0; g = 1; r = 1.0e-300; each();
  end
endmodule
