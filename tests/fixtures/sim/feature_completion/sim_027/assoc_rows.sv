// SIM-027 A03: associative arrays whose elements are unpacked arrays as
// memory_name (IEEE 1800-2009 21.4, 21.4.1, 21.4.3, 21.5.3; decisions
// S27-D22..D25). The whole array: an address is a key, cast to the index type,
// and the words after it fill that element's cells in row-major order before
// the key moves on; a load creates the elements it reaches, a dump writes each
// element as its `@` key and its cells. One element (`aa[k]`, also with an
// index or slice inside it): a fixed unpacked array whose addresses are its own
// indices; a load creates a missing key once the file and range are valid, a
// dump of a missing or unknown key warns and writes the default value without
// creating the element.
module tb;
  typedef enum logic [3:0] { RED = 1, GREEN = 4 } color_t;
  class holder;
    byte rows [int][2];
    task go();
      $readmemh("p.mem", rows);
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
    $readmemh("p.mem", loc, 3);
    $readmemh("p.mem", loc[9]);
    $display("local %p", loc);
    $writememh("loc.mem", loc);
    show_file("loc.mem");
  endtask

  initial begin
    make_file("w.mem", "11 12 13 14 15\n@7\n21 22 23\n");
    make_file("v.mem", "aa bb cc dd\n");
    make_file("r.mem", "@2\n01 02 03\n@9\n04\n");
    make_file("f.mem", "31 32 33 34\n");
    make_file("p.mem", "0a 0b\n");
    make_file("g.mem", "01 02 03 04 05 06 07 08 09\n");
    make_file("n.mem", "01 02 03 04 05 06\n");
    make_file("n2.mem", "0f\n");
    make_file("blank.mem", "");
    make_file("miss.mem", "keep\n");
    unk = 'x;

    // The default start is key 0; a short last row keeps default cells and
    // an address resets the position to the first cell of its key.
    $readmemh("w.mem", aa);
    $display("A %p n=%0d", aa, aa.num());
    // A start key updates only the cells it reaches, creating key 2.
    $readmemh("v.mem", aa, 1);
    $display("B %p n=%0d", aa, aa.num());
    // A descending range moves from key 5 toward key 4.
    $readmemh("v.mem", aa, 5, 4);
    $display("C %p n=%0d", aa, aa.num());
    // A file address must lie within the requested keys.
    $readmemh("r.mem", aa, 1, 2);
    $display("D %p n=%0d", aa, aa.num());
    // The last requested key's row ends the load.
    $readmemh("f.mem", aa, 0, 0);
    $display("E %p n=%0d", aa, aa.num());

    // Dumps: all elements, a descending range, a start key and binary.
    $writememh("o.mem", aa);
    show_file("o.mem");
    $writememh("o2.mem", aa, 5, 2);
    show_file("o2.mem");
    $writememh("o3.mem", aa, 5);
    show_file("o3.mem");
    $writememb("ob.mem", aa, 4, 4);
    show_file("ob.mem");

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
    $readmemh("p.mem", desc);
    $display("H %p", desc);
    $writememh("hd.mem", desc);
    show_file("hd.mem");

    // A two-dimensional element, filled row-major; the byte index wraps
    // after key 255.
    $readmemh("g.mem", grid, 254);
    $display("I %p", grid);
    $writememh("gr.mem", grid, 255);
    show_file("gr.mem");

    // A signed index reads negative keys; new elements start unknown.
    $readmemh("n.mem", lg, -2);
    $readmemh("n2.mem", lg, 5);
    $display("J %p", lg);
    $writememh("ln.mem", lg);
    show_file("ln.mem");

    // Enumerated index: numeric keys of the enum values.
    $readmemh("p.mem", by_color, GREEN);
    $readmemh("p.mem", by_color[RED]);
    $writememh("co.mem", by_color);
    show_file("co.mem");

    // An empty file loads nothing; one element is still created by its
    // load once the file opened.
    $readmemh("blank.mem", empty);
    $display("L n=%0d", empty.num());
    $readmemh("blank.mem", empty[2]);
    $display("L n=%0d %p", empty.num(), empty);
    $writememh("em.mem", empty);
    show_file("em.mem");

    h = new;
    h.go();
    show_file("h.mem");
    local_load();
    $finish;
  end
endmodule
