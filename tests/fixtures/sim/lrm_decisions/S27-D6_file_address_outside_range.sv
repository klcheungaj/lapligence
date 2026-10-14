// Decision S27-D6: with start and finish addresses given, a file address
// outside that range is an error that terminates the load. The words loaded
// before it stay, the rest of the range keeps its values, and the
// simulation continues with the next statement.
//
// IEEE 1800-2009 21.4 (SystemVerilog-1800-2009.txt L37315-37317):
//   "When addressing information is specified both in the system task and in
//   the data file, the addresses in the data file shall be within the address
//   range specified by the system task arguments; otherwise, an error message
//   is issued, and the load operation is terminated."
module tb;
  logic [7:0] mem [0:7];
  integer fd;
  initial begin
    fd = $fopen("s27_d6.mem", "w");
    $fwrite(fd, "11\n22\n@6\n33\n44\n");
    $fclose(fd);
    mem = '{default: 8'hee};
    $readmemh("s27_d6.mem", mem, 1, 4);
    $display("mem %h %h %h %h %h %h %h %h", mem[0], mem[1], mem[2], mem[3], mem[4],
             mem[5], mem[6], mem[7]);
    $finish;
  end
endmodule
