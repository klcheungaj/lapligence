// Real elements are legal for reverse in IEEE 1800-2009 7.12.2, but outside
// llg's selected fixed integral representation.
module tb;
  real values [0:1];
  initial begin
    values.reverse();
  end
endmodule
