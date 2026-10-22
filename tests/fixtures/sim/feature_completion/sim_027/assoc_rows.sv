// SIM-027 A03: elements of associative arrays whose elements are unpacked
// arrays as memory_name (IEEE 1800-2009 21.4, 21.4.1, 21.4.3, 21.5.3;
// decisions S27-D23..D25). One element (`aa[k]`, also with an index or slice
// inside it) is a fixed unpacked array whose addresses are its own indices; a
// load creates a missing key once the file and range are valid, a dump writes
// the cells without `@` addresses, and a dump of a missing or unknown key
// warns and writes the default value without creating the element. The whole
// array is not a memory_name (S27-D22).
module tb;
  typedef enum logic [3:0] { RED = 1, GREEN = 4 } color_t;
  class holder;
    byte rows [int][2];
    task go();
      $readmemh("p.mem", rows[0]);
      $readmemh("p.mem", rows[4]);
      $writememh("h.mem", rows[4]);
      $display("class %p", rows);
    endtask
  endclass

  bit [7:0] aa [int][3];
  logic [7:0] lg [shortint][2];
  logic [7:0] grid [byte unsigned][2][2];
  logic [7:0] desc [int][1:0];
  logic [7:0] by_color [color_t][2];
  logic [7:0] empty [int][2];
  integer unk;
  holder h;
  int fd;

  task automatic show_file(string name);
    string line;
    int f;
    $display("%s:", name);
    f = $fopen(name, "r");
    while ($fgets(line, f) != 0) $write("  %s", line);
    $fclose(f);
  endtask

  task automatic make_file(string name, string text);
    int f;
    f = $fopen(name, "w");
    $fwrite(f, "%s", text);
    $fclose(f);
  endtask

  task automatic local_load;
    bit [7:0] loc [int][2];
    $readmemh("p.mem", loc[3]);
    $readmemh("p.mem", loc[9]);
    $display("local %p", loc);
    $writememh("loc.mem", loc[9]);
    show_file("loc.mem");
  endtask

  initial begin
    make_file("w.mem", "11 12 13\n");
    make_file("v.mem", "aa bb cc dd\n");
    make_file("p.mem", "0a 0b\n");
    make_file("g.mem", "01 02 03 04\n");
    make_file("n.mem", "01 02\n");
    make_file("n2.mem", "0f\n");
    make_file("blank.mem", "");
    make_file("miss.mem", "keep\n");
    unk = 'x;

    // Elements created by their loads: a short file keeps the default cells,
    // a start and finish address update only the cells they name.
    $readmemh("w.mem", aa[0]);
    $readmemh("p.mem", aa[1]);
    $readmemh("v.mem", aa[1], 1, 2);
    $display("A %p n=%0d", aa, aa.num());

    // One element: created when missing, then an element-relative slice and
    // start address.
    $readmemh("v.mem", aa[3]);
    $readmemh("p.mem", aa[3][1+:2]);
    $readmemh("p.mem", aa[6], 1);
    // An address outside the element or an unknown key creates nothing.
    $readmemh("p.mem", aa[8], 5);
    $readmemh("p.mem", aa[unk]);
    $display("G %p n=%0d exists8=%0d", aa, aa.num(), aa.exists(8));
    // Dumping an element writes its cells without `@` addresses.
    $writememh("e1.mem", aa[1]);
    show_file("e1.mem");
    $writememh("e3.mem", aa[3][1+:2]);
    show_file("e3.mem");
    $writememh("e6.mem", aa[6], 2);
    show_file("e6.mem");
    // A missing or unknown key writes the default value and creates nothing.
    $writememh("miss.mem", aa[99]);
    show_file("miss.mem");
    $writememh("miss.mem", aa[unk], 1);
    show_file("miss.mem");
    $display("missing n=%0d exists99=%0d", aa.num(), aa.exists(99));

    // A descending element dimension: address 0 is its element [0].
    $readmemh("p.mem", desc[0]);
    $display("H %p", desc);
    $writememh("hd.mem", desc[0]);
    show_file("hd.mem");

    // A two-dimensional element, filled row-major, at the largest key of a
    // byte unsigned index.
    $readmemh("g.mem", grid[255]);
    $display("I %p", grid);
    $writememh("gr.mem", grid[255], 1);
    show_file("gr.mem");

    // A negative key of a signed index; new four-state cells start unknown.
    $readmemh("n.mem", lg[-2]);
    $readmemh("n2.mem", lg[5], 0, 0);
    $display("J %p", lg);
    $writememh("ln.mem", lg[5]);
    show_file("ln.mem");

    // Enumerated index: one element per enum value.
    $readmemh("p.mem", by_color[GREEN]);
    $readmemh("n.mem", by_color[RED]);
    $writememh("co.mem", by_color[GREEN]);
    show_file("co.mem");
    $writememh("co.mem", by_color[RED]);
    show_file("co.mem");

    // An empty file loads nothing; the element is still created by its load
    // once the file opened.
    $readmemh("blank.mem", empty[2]);
    $display("L n=%0d %p", empty.num(), empty);
    $writememh("em.mem", empty[2]);
    show_file("em.mem");

    h = new;
    h.go();
    show_file("h.mem");
    local_load();
    $finish;
  end
endmodule
