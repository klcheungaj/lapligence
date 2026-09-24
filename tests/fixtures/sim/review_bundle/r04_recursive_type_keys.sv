// IEEE 1800-2009 10.9.1: descend into record elements for type/default keys.
module tb;
  typedef struct { int count; bit flag; } record_t;
  record_t values[0:1];
  int seed;
  initial begin
    seed = 17;
    values = '{int:seed, default:'0};
    if (values[0].count !== 17 || values[1].count !== 17 ||
        values[0].flag !== 1'b0 || values[1].flag !== 1'b0)
      $fatal(1, "recursive type key");
    $display("PASS r04_recursive_type_keys");
    $finish;
  end
endmodule
