// The same restriction for an automatic memory, which loads through its image.
module tb;
  task automatic load;
    real mem [0:3];
    $readmemh("in.hex", mem);
  endtask
  initial load;
endmodule
