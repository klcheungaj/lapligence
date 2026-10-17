// ADV-026 step 1: every memory_name storage form that $readmemh accepts is a
// legal memory_name of $sreadmemh (IEEE 1800-2009 D.14: "load data into
// memory mem_name"; the strings "take the same format as the strings that
// appear in the input files"; 21.4 and 21.4.1 define the storage forms). The
// words are those of sim_027's contexts.sv, given as separate strings, so every
// printed line equals the one the file-based fixture derives (hand-checked in
// the readme); each call is given exactly the words of its range.
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

  task automatic show4(string label, logic [7:0] a, b, c, d);
    $display("%s=%h,%h,%h,%h", label, a, b, c, d);
  endtask

  task static static_local;
    logic [7:0] cells [0:3];
    $sreadmemh(cells, 0, 3, "10 11", "12 13");
    show4("static_local", cells[0], cells[1], cells[2], cells[3]);
  endtask

  task automatic automatic_local;
    logic [7:0] cells [3:0];
    $sreadmemh(cells, 3, 0, "20 21", "22 23");
    show4("automatic_local[3:0]", cells[3], cells[2], cells[1], cells[0]);
  endtask

  function automatic int function_sum();
    logic [7:0] cells [0:3];
    int sum = 0;
    $sreadmemh(cells, 0, 3, "30 31", "32 33");
    foreach (cells[i]) sum += cells[i];
    return sum;
  endfunction

  task automatic load_ref(ref logic [7:0] cells [0:3]);
    $sreadmemh(cells, 0, 3, "50 51", "52 53");
  endtask

  task automatic ref_of_automatic;
    logic [7:0] mine [0:3];
    load_ref(mine);
    show4("ref_automatic", mine[0], mine[1], mine[2], mine[3]);
  endtask

  task automatic local_views;
    logic [7:0] cells [0:2][0:3];
    int row;
    foreach (cells[i, j]) cells[i][j] = 8'hee;
    row = 2;
    $sreadmemh(cells[1], 0, 3, "60 61", "62 63");
    $sreadmemh(cells[row], 3, 2, "70", "71");
    $sreadmemh(cells[0][1:2], 1, 2, "80", "81");
    show4("local_row0", cells[0][0], cells[0][1], cells[0][2], cells[0][3]);
    show4("local_row1", cells[1][0], cells[1][1], cells[1][2], cells[1][3]);
    show4("local_row2", cells[2][0], cells[2][1], cells[2][2], cells[2][3]);
  endtask

  initial begin
    holder h;

    $sreadmemh(module_mem, 0, 3, "40 41", "42 43");
    show4("module", module_mem[0], module_mem[1], module_mem[2], module_mem[3]);
    static_local;
    automatic_local;
    $display("function_sum=%0d", function_sum());
    load_ref(module_mem);
    show4("ref_module", module_mem[0], module_mem[1], module_mem[2], module_mem[3]);
    ref_of_automatic;
    local_views;

    begin
      automatic logic [7:0] block_cells [0:3];
      $sreadmemh(block_cells, 0, 3, "90 91", "92 93");
      show4("block_automatic", block_cells[0], block_cells[1], block_cells[2], block_cells[3]);
      fork
        $sreadmemh(block_cells, 0, 3, "a0 a1", "a2 a3");
      join
      show4("fork_shared", block_cells[0], block_cells[1], block_cells[2], block_cells[3]);
    end

    record.tag = 7;
    $sreadmemh(record.cells, 0, 3, "b0 b1", "b2 b3");
    show4("struct_member", record.cells[0], record.cells[1], record.cells[2], record.cells[3]);
    $display("struct_tag=%0d", record.tag);

    h = new;
    $sreadmemh(h.cells, 2, 5, "c0 c1", "c2 c3");
    show4("class_property[5:2]", h.cells[5], h.cells[4], h.cells[3], h.cells[2]);
    $sreadmemh(h.cells[4:3], 3, 4, "10 11");
    show4("class_slice", h.cells[5], h.cells[4], h.cells[3], h.cells[2]);
    $sreadmemh(holder::shared, 0, 3, "20 21", "22 23");
    show4("static_property", holder::shared[0], holder::shared[1], holder::shared[2], holder::shared[3]);

    $sreadmemh(memory_pkg::pkg_mem, 0, 3, "30 31", "32 33");
    show4("package", memory_pkg::pkg_mem[0], memory_pkg::pkg_mem[1], memory_pkg::pkg_mem[2], memory_pkg::pkg_mem[3]);
    $sreadmemh(u.hier_mem, 1, 2, "40", "41");
    show4("hierarchical", u.hier_mem[0], u.hier_mem[1], u.hier_mem[2], u.hier_mem[3]);
    $sreadmemh(grid[1], 2, 3, "50", "51");
    show4("grid_row1", grid[1][0], grid[1][1], grid[1][2], grid[1][3]);

    dyn = new[3];
    fifo = '{8'h01, 8'h02, 8'h03, 8'h04, 8'h05};
    $sreadmemh(dyn, 0, 2, "60 61", "62");
    $sreadmemh(fifo, 4, 2, "70 71", "72");
    $display("dynamic=%h,%h,%h size=%0d", dyn[0], dyn[1], dyn[2], dyn.size());
    $display("queue=%h,%h,%h,%h,%h size=%0d", fifo[0], fifo[1], fifo[2], fifo[3], fifo[4], fifo.size());
    $finish;
  end
endmodule
