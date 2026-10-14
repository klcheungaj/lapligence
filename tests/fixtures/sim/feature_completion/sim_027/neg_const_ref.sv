// A `const ref` formal cannot be the target of a load (IEEE 1800-2009 13.5.2).
module tb;
  logic [7:0] g [0:3];
  task automatic load(const ref logic [7:0] mem [0:3]);
    $readmemh("in.hex", mem);
  endtask
  initial load(g);
endmodule
