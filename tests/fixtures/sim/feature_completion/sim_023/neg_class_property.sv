// SV 6.21: class properties shall not be written with procedural continuous
// assignments.
class C;
  logic [3:0] f;
endclass
module tb;
  C c;
  initial begin
    c = new;
    force c.f = 4'h1;
  end
endmodule
