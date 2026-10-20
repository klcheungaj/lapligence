`timescale 1ns/1ns
// ADV-007 A01: path delays mixed with distributed delays (the larger one
// wins), with the destination's own net delay, with a destination driven by
// a child instance, and with an inout destination driven only inside.
module fig3(input D, output Q);
  wire n;
  buf #0 g1(n, D);
  buf #1 g2(Q, n);
  specify
    (D => Q) = 22;
  endspecify
endmodule

module fig4(input D, output Q);
  wire n;
  buf #10 g1(n, D);
  buf #20 g2(Q, n);
  specify
    (D => Q) = 22;
  endspecify
endmodule

module netd(D, Q);
  input D;
  output Q;
  wire #4 Q;
  assign Q = D;
  specify
    (D => Q) = 2;
  endspecify
endmodule

module netp(D, Q);
  input D;
  output Q;
  wire #1 Q;
  assign Q = D;
  specify
    (D => Q) = 6;
  endspecify
endmodule

module inv(input a, output y);
  assign y = ~a;
endmodule

module hier(input D, output Q);
  inv i(.a(D), .y(Q));
  specify
    (D => Q) = 4;
  endspecify
endmodule

module io(input D, inout Q);
  assign Q = D;
  specify
    (D => Q) = 3;
  endspecify
endmodule

module tb;
  reg D;
  wire q3, q4, qn, qp, qh, qi, qr;
  fig3 u3(.D(D), .Q(q3));
  fig4 u4(.D(D), .Q(q4));
  netd un(.D(D), .Q(qn));
  netp up(.D(D), .Q(qp));
  hier uh(.D(D), .Q(qh));
  io ui(.D(D), .Q(qi));
  assign qr = qi;
  initial $monitor("%0t D=%b q3=%b q4=%b qn=%b qp=%b qh=%b qi=%b qr=%b",
                   $time, D, q3, q4, qn, qp, qh, qi, qr);
  initial begin
    D = 0;
    #100 D = 1;
    #100 D = 0;
    #100 $finish(0);
  end
endmodule
