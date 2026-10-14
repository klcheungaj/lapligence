// Decision S27-D17: a dump of an associative array writes its existing
// entries in index order (signed for a signed index), each word preceded by
// its own @ address in lowercase hexadecimal without leading zeros (the
// index's bits, two's complement for a negative index). A start address
// alone keeps the entries from it upward; start and finish keep the entries
// in that inclusive range, in the start-to-finish direction. An empty array
// dumps as an empty file.
//
// IEEE 1800-2009 21.5.3 (SystemVerilog-1800-2009.txt L37500-37502):
//   "When $writememb and $writememh write out data corresponding to
//   associative arrays, address specifiers shall be written to the output
//   file. As specified in 21.4.1, associative arrays shall have indices of
//   integral types in order to be legal arguments to the $writememb and
//   $writememh calls."
// IEEE 1800-2009 7.8.4 (L8699):
//   "The ordering is signed or unsigned numerical, depending on the
//   signedness of the index type."
//
// The text requires the address specifiers but not where they appear or how
// they are spelled; one per word, so that the file reloads in any direction,
// is an llg choice.
module tb;
  logic [7:0] table_ [int];
  logic [7:0] empty [int];
  task automatic show(string name);
    string line;
    integer f;
    f = $fopen(name, "r");
    $display("%s:", name);
    while ($fgets(line, f) != 0) $write("  %s", line);
    $fclose(f);
  endtask
  initial begin
    table_[20] = 8'h14;
    table_[-2] = 8'hfe;
    table_[3] = 8'h03;
    table_[4] = 8'h04;
    $writememh("s27_d17_all.mem", table_);
    show("s27_d17_all.mem");
    $writememh("s27_d17_from.mem", table_, 4);
    show("s27_d17_from.mem");
    $writememh("s27_d17_down.mem", table_, 20, 3);
    show("s27_d17_down.mem");
    $writememb("s27_d17_bin.mem", table_, 3, 4);
    show("s27_d17_bin.mem");
    $writememh("s27_d17_empty.mem", empty);
    show("s27_d17_empty.mem");
    $finish;
  end
endmodule
