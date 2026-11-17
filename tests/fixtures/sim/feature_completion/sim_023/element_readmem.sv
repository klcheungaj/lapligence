// SV 10.6.2, 21.4: `$readmemh` writes every element of the array but the
// forced one, which keeps its forced value until it is released and written.
module tb;
  logic [7:0] mem[4];
  logic [7:0] big[100000];
  initial begin
    force mem[1] = 8'hee;
    force big[2] = 8'hdd;
    $readmemh("element_readmem.hex", mem);
    $readmemh("element_readmem.hex", big, 0, 3);
    #1 $display("r1 %h %h %h %h", mem[0], mem[1], mem[2], mem[3]);
    $display("r2 %h %h %h %h", big[0], big[1], big[2], big[3]);
    release mem[1];
    release big[2];
    $readmemh("element_readmem.hex", mem);
    $readmemh("element_readmem.hex", big, 0, 3);
    #1 $display("r3 %h %h", mem[1], big[2]);
    $display("done");
    $finish;
  end
endmodule
