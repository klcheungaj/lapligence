// memory_name must remain an unpacked array; indexing every dimension names
// one word (IEEE 1800-2009 21.4).
module tb;
  task automatic load;
    logic [7:0] mem [0:3];
    $readmemh("in.hex", mem[1]);
  endtask
  initial load;
endmodule
