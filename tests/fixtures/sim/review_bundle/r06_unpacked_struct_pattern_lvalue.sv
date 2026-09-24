// Unpacked fixed integral structures are also positional pattern targets.
module tb;
  typedef struct { logic [7:0] hi; logic [7:0] lo; } pair_t;
  pair_t source;
  logic [7:0] a,b;
  initial begin
    source.hi = 8'h12;
    source.lo = 8'h34;
    pair_t'{a,b} = source;
    if (a !== 8'h12 || b !== 8'h34) $fatal(1, "structure deconstruction");
    $display("PASS r06_unpacked_struct_pattern_lvalue");
    $finish;
  end
endmodule
