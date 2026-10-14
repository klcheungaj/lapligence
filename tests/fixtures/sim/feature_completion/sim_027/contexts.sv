// SIM-027 A03: every legal memory_name storage form loads and dumps through
// $readmemh/$writememh (IEEE 1800-2009 21.4, 21.4.1, 21.5). Each context
// first writes its own data file, so the run needs no input files.
package memory_pkg;
  logic [7:0] pkg_mem [0:3];
endpackage

module sub;
  logic [7:0] hier_mem [0:3];
endmodule

module tb;
  typedef struct { logic [7:0] cells [0:3]; int tag; } record_t;
  class holder;
    logic [7:0] cells [5:2];
    static logic [7:0] shared [0:3];
  endclass

  sub u();
  logic [7:0] module_mem [0:3];
  logic [7:0] grid [0:2][0:3];
  record_t record;
  logic [7:0] dyn [];
  logic [7:0] fifo [$];

  // Writes four words a, a+1, a+2, a+3 (hex) to `name`.
  task automatic make_file(string name, int a);
    int f;
    f = $fopen(name, "w");
    $fwrite(f, "%h\n%h\n%h\n%h\n", a[7:0], a[7:0] + 8'd1, a[7:0] + 8'd2, a[7:0] + 8'd3);
    $fclose(f);
  endtask

  task automatic show4(string label, logic [7:0] a, b, c, d);
    $display("%s=%h,%h,%h,%h", label, a, b, c, d);
  endtask

  task static static_local;
    logic [7:0] cells [0:3];
    $readmemh("d10.mem", cells);
    show4("static_local", cells[0], cells[1], cells[2], cells[3]);
    $writememh("static_local.mem", cells);
  endtask

  task automatic automatic_local;
    logic [7:0] cells [3:0];
    $readmemh("d20.mem", cells);
    show4("automatic_local[3:0]", cells[3], cells[2], cells[1], cells[0]);
    $writememh("automatic_local.mem", cells, 1, 2);
  endtask

  function automatic int function_sum();
    logic [7:0] cells [0:3];
    int sum = 0;
    $readmemh("d30.mem", cells);
    foreach (cells[i]) sum += cells[i];
    return sum;
  endfunction

  task automatic load_ref(ref logic [7:0] cells [0:3], input string name);
    $readmemh(name, cells);
  endtask

  task automatic dump_ref(const ref logic [7:0] cells [0:3], input string name);
    $writememh(name, cells);
  endtask

  task automatic ref_of_automatic;
    logic [7:0] mine [0:3];
    load_ref(mine, "d50.mem");
    show4("ref_automatic", mine[0], mine[1], mine[2], mine[3]);
  endtask

  task automatic local_views;
    logic [7:0] cells [0:2][0:3];
    int row;
    foreach (cells[i, j]) cells[i][j] = 8'hee;
    row = 2;
    $readmemh("d60.mem", cells[1]);
    $readmemh("d70.mem", cells[row], 3, 2);
    $readmemh("d80.mem", cells[0][1:2]);
    show4("local_row0", cells[0][0], cells[0][1], cells[0][2], cells[0][3]);
    show4("local_row1", cells[1][0], cells[1][1], cells[1][2], cells[1][3]);
    show4("local_row2", cells[2][0], cells[2][1], cells[2][2], cells[2][3]);
  endtask

  initial begin
    holder h;
    make_file("d10.mem", 'h10);
    make_file("d20.mem", 'h20);
    make_file("d30.mem", 'h30);
    make_file("d40.mem", 'h40);
    make_file("d50.mem", 'h50);
    make_file("d60.mem", 'h60);
    make_file("d70.mem", 'h70);
    make_file("d80.mem", 'h80);
    make_file("d90.mem", 'h90);
    make_file("da0.mem", 'ha0);
    make_file("db0.mem", 'hb0);
    make_file("dc0.mem", 'hc0);

    $readmemh("d40.mem", module_mem);
    show4("module", module_mem[0], module_mem[1], module_mem[2], module_mem[3]);
    static_local;
    automatic_local;
    $display("function_sum=%0d", function_sum());
    load_ref(module_mem, "d50.mem");
    show4("ref_module", module_mem[0], module_mem[1], module_mem[2], module_mem[3]);
    ref_of_automatic;
    dump_ref(module_mem, "ref_dump.mem");
    local_views;

    begin
      automatic logic [7:0] block_cells [0:3];
      $readmemh("d90.mem", block_cells);
      show4("block_automatic", block_cells[0], block_cells[1], block_cells[2], block_cells[3]);
      fork
        $readmemh("da0.mem", block_cells);
      join
      show4("fork_shared", block_cells[0], block_cells[1], block_cells[2], block_cells[3]);
    end

    record.tag = 7;
    $readmemh("db0.mem", record.cells);
    show4("struct_member", record.cells[0], record.cells[1], record.cells[2], record.cells[3]);
    $display("struct_tag=%0d", record.tag);

    h = new;
    $readmemh("dc0.mem", h.cells);
    show4("class_property[5:2]", h.cells[5], h.cells[4], h.cells[3], h.cells[2]);
    $readmemh("d10.mem", h.cells[4:3]);
    show4("class_slice", h.cells[5], h.cells[4], h.cells[3], h.cells[2]);
    $readmemh("d20.mem", holder::shared);
    show4("static_property", holder::shared[0], holder::shared[1], holder::shared[2], holder::shared[3]);
    $writememh("class.mem", h.cells);

    $readmemh("d30.mem", memory_pkg::pkg_mem);
    show4("package", memory_pkg::pkg_mem[0], memory_pkg::pkg_mem[1], memory_pkg::pkg_mem[2], memory_pkg::pkg_mem[3]);
    $readmemh("d40.mem", u.hier_mem, 1, 2);
    show4("hierarchical", u.hier_mem[0], u.hier_mem[1], u.hier_mem[2], u.hier_mem[3]);
    $readmemh("d50.mem", grid[1], 2);
    show4("grid_row1", grid[1][0], grid[1][1], grid[1][2], grid[1][3]);

    dyn = new[3];
    fifo = '{8'h01, 8'h02, 8'h03, 8'h04, 8'h05};
    $readmemh("d60.mem", dyn);
    $readmemh("d70.mem", fifo, 4, 2);
    $display("dynamic=%h,%h,%h size=%0d", dyn[0], dyn[1], dyn[2], dyn.size());
    $display("queue=%h,%h,%h,%h,%h size=%0d", fifo[0], fifo[1], fifo[2], fifo[3], fifo[4], fifo.size());
    $writememh("queue.mem", fifo);

    // Read every dump back into a module memory: the round trip is exact.
    module_mem = '{default: 8'h00};
    $readmemh("static_local.mem", module_mem);
    show4("dump_static_local", module_mem[0], module_mem[1], module_mem[2], module_mem[3]);
    module_mem = '{default: 8'h00};
    $readmemh("automatic_local.mem", module_mem, 1, 2);
    show4("dump_automatic_local", module_mem[0], module_mem[1], module_mem[2], module_mem[3]);
    module_mem = '{default: 8'h00};
    $readmemh("ref_dump.mem", module_mem);
    show4("dump_ref", module_mem[0], module_mem[1], module_mem[2], module_mem[3]);
    module_mem = '{default: 8'h00};
    $readmemh("class.mem", module_mem);
    show4("dump_class", module_mem[0], module_mem[1], module_mem[2], module_mem[3]);
    begin
      logic [7:0] five [0:4];
      $readmemh("queue.mem", five);
      $display("dump_queue=%h,%h,%h,%h,%h", five[0], five[1], five[2], five[3], five[4]);
    end
    $finish;
  end
endmodule
