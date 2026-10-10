// IEEE 1800-2009 11.11: a real already converts to int by assignment, so an
// `=` overload from real to int would replace a legal operation.
module tb;
  function automatic int r2i(real r);
    return 7;
  endfunction
  bind = function int r2i(real);
  int i;
  initial i = 2.5;
endmodule
