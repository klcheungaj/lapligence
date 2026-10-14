// Decision S27-D23: one element of an associative array of unpacked arrays
// (aa[k]) is a memory_name for a fixed unpacked array, whose addresses are its
// own indices (a start address, or a slice of it, selects cells inside the
// element). A load creates the element k, with the default value of its
// type, when it is missing and updates only the cells it reaches when it
// exists. The element is created once the file has opened and the arguments
// are valid: a file that cannot be opened, an address outside the element
// and an unknown key (X or Z bits) load nothing and create nothing.
//
// IEEE 1800-2009 21.4.1 (SystemVerilog-1800-2009.txt L37359-37360):
//   "Loading an address creates an element of that index, if it does not
//   already exist."
// IEEE 1800-2009 21.4 (L37251-37254):
//   "The memory_name can be an unpacked array or a partially indexed
//   multidimensional unpacked array that resolves to a lesser dimensioned
//   unpacked array (see 21.4.3). Higher order dimensions shall be specified
//   with an index, rather than a complete or partial dimension range."
// IEEE 1800-2009 7.8.6 (L8722-8724, L8756-8757):
//   "If an invalid index (i.e., 4-state expression has X's) is used during a
//   read operation or an attempt is made to read a nonexistent entry, then a
//   warning shall be issued ... If an invalid index is used during a write
//   operation, the write shall be ignored, and a warning shall be issued."
//
// The text is silent on when the element of a load appears relative to the
// file and the range; creating it after both are known to be usable, so a
// failed call has no effect, is an llg choice. The cells of the element are
// shown through its dump (S27-D25).
module tb;
  logic [7:0] aa [int][3];
  integer k;

  task make(input string name, input string text);
    integer f;
    f = $fopen(name, "w");
    $fwrite(f, "%s", text);
    $fclose(f);
  endtask

  task show(input string name);
    string line;
    integer f;
    f = $fopen(name, "r");
    $display("%s:", name);
    while ($fgets(line, f) != 0) $write("  %s", line);
    $fclose(f);
  endtask

  task state(input string label);
    $display("%s elements %0d exists5 %0d exists6 %0d exists7 %0d exists8 %0d", label,
             aa.num(), aa.exists(5), aa.exists(6), aa.exists(7), aa.exists(8));
  endtask

  initial begin
    make("s27_d23_two.mem", "0a 0b\n");
    make("s27_d23_one.mem", "0c\n");
    state("start");
    // Missing: created, with the cells the file does not reach unknown.
    $readmemh("s27_d23_two.mem", aa[5]);
    $writememh("s27_d23_a.mem", aa[5]);
    show("s27_d23_a.mem");
    // Existing: only the cell at the start address changes.
    $readmemh("s27_d23_one.mem", aa[5], 2);
    $writememh("s27_d23_b.mem", aa[5]);
    show("s27_d23_b.mem");
    // A slice inside a missing element creates it.
    $readmemh("s27_d23_two.mem", aa[6][1+:2]);
    $writememh("s27_d23_c.mem", aa[6]);
    show("s27_d23_c.mem");
    state("loaded");
    // Nothing is created for an address outside the element, an unknown key
    // or a file that cannot be opened.
    $readmemh("s27_d23_two.mem", aa[7], 9);
    k = 'x;
    $readmemh("s27_d23_two.mem", aa[k]);
    $readmemh("s27_d23_missing_file.mem", aa[8]);
    state("rejected");
    $finish;
  end
endmodule
