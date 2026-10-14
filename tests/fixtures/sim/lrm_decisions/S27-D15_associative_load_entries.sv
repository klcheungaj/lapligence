// Decision S27-D15: loading an associative array with an integral index
// creates the entry at each address or updates an existing one; entries the
// file does not address keep their values. Without addressing information
// the first word goes to index 0. Without a finish address the index
// advances upward until the file ends; with a start and a finish address it
// moves from start toward finish, a file address outside that range is an
// error that ends the load, and a file with fewer words than the range draws
// the 21.4 word-count warning.
//
// IEEE 1800-2009 21.4.1 (SystemVerilog-1800-2009.txt L37359-37360):
//   "When loading associative arrays, indices shall be of integral types.
//   Loading an address creates an element of that index, if it does not
//   already exist."
// IEEE 1800-2009 21.4 (L37303-37305):
//   "If no addressing information is specified within the system task and no
//   address specifications appear within the data file, then the default
//   start address shall be the lowest address in the memory. Consecutive
//   words shall be loaded until either the highest address in the memory is
//   reached or the data file is completely read."
//
// An associative array has no lowest address of its own; llg starts at index
// 0 (an llg choice). The range rules are those 21.4 states for any memory.
module tb;
  logic [7:0] table_ [int];
  logic [7:0] ranged [int];
  integer fd;
  initial begin
    fd = $fopen("s27_d15.mem", "w");
    $fwrite(fd, "11\n22\n@8\n33\n");
    $fclose(fd);
    fd = $fopen("s27_d15_short.mem", "w");
    $fwrite(fd, "aa\nbb\n");
    $fclose(fd);
    table_[1] = 8'h99;
    table_[5] = 8'h55;
    $readmemh("s27_d15.mem", table_);
    foreach (table_[k]) $display("default [%0d]=%h", k, table_[k]);
    table_.delete();
    $readmemh("s27_d15.mem", table_, -3);
    foreach (table_[k]) $display("start -3 [%0d]=%h", k, table_[k]);
    $readmemh("s27_d15_short.mem", ranged, 7, 4);
    foreach (ranged[k]) $display("7 to 4 [%0d]=%h", k, ranged[k]);
    ranged.delete();
    $readmemh("s27_d15.mem", ranged, 0, 4);
    foreach (ranged[k]) $display("0 to 4 [%0d]=%h", k, ranged[k]);
    $finish;
  end
endmodule
