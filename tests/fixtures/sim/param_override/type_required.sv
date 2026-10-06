// A top-level type parameter without a default is legal only when the command
// line supplies the type; the value parameter's type follows it.
module tb #(parameter type T, parameter T V = 0);
  T copy = V;
  initial $display("bits=%0d V=%0d copy=%b", $bits(T), V, copy);
endmodule
