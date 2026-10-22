// Output port expressions that concatenate variables and nets, connected in
// order and by name to an outside net and an outside variable.
module mv(.p({a, b}));
  output logic a;
  output logic [1:0] b;
  initial begin a = 1; b = 2'b01; #5 a = 0; b = 2'b10; end
endmodule

module mn(.p({a, b}), .q(c));
  output a;
  output [1:0] b;
  input [2:0] c;
  assign a = c[0];
  assign b = c[2:1];
endmodule

module tb;
  wire [2:0] pm;
  logic [2:0] pv;
  wire [2:0] nm;
  logic [2:0] c;
  mv m1(.p(pm));
  mv m2(pv);
  mn m3(.q(c), .p(nm));
  initial begin
    c = 3'b011;
    #1 $display("1 pm=%b pv=%b a=%b b=%b nm=%b", pm, pv, m1.a, m1.b, nm);
    c = 3'b100;
    #5 $display("6 pm=%b pv=%b a=%b b=%b nm=%b", pm, pv, m1.a, m1.b, nm);
    $finish;
  end
endmodule
