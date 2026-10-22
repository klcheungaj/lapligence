// Port expressions in interface and program headers.
interface ifc(.p({a, b}), .q(c[1:0]));
  input a;
  input b;
  output logic [3:0] c;
  initial c = 4'b1110;
endinterface

program prg(.p({a, b}), .q(c[2:1]));
  input a;
  input b;
  output logic [3:0] c;
  initial begin
    c = 4'b0100;
    // Program processes run in the Reactive region, after the module's
    // display at the same time.
    #1 $display("prg a=%b b=%b", a, b);
  end
endprogram

module tb;
  logic [1:0] d;
  wire [1:0] q1, q2;
  ifc i1(.p(d), .q(q1));
  prg p1(.p(~d), .q(q2));
  initial begin
    d = 2'b10;
    #1 $display("ifc a=%b b=%b q1=%b q2=%b", i1.a, i1.b, q1, q2);
    #1 $finish;
  end
endmodule
