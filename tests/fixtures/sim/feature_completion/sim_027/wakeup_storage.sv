// SIM-027 A02: the same update rule for memories that the runtime loads
// through a packed image: a task's `ref` formal bound to a module memory, a
// structure member and a static task memory (IEEE 1800-2009 4.3, 13.5.2,
// 21.4). Each process waits on one word; only the processes whose word
// changes wake, once, in the load's time slot. Their order is not fixed
// (4.7); `done` comes last.
module tb;
  typedef struct { logic [7:0] cells [0:2]; logic [7:0] tag; } record_t;
  logic [7:0] shared [0:2];
  record_t record;

  task automatic load(ref logic [7:0] cells [0:2]);
    $readmemh("wake.hex", cells);
  endtask

  task static keep;
    logic [7:0] cells [0:2];
    $readmemh("wake.hex", cells);
  endtask

  always @(shared[0]) if ($time > 0) $display("ref0=%h", shared[0]);
  always @(shared[1]) if ($time > 0) $display("ref1=%h", shared[1]);
  always @(shared[2]) if ($time > 0) $display("ref2=%h", shared[2]);
  always @(record.cells[0]) if ($time > 0) $display("member0=%h", record.cells[0]);
  always @(record.cells[1]) if ($time > 0) $display("member1=%h", record.cells[1]);
  always @(record.tag) if ($time > 0) $display("tag=%h", record.tag);
  always @(keep.cells[2]) if ($time > 0) $display("static2=%h", keep.cells[2]);
  always @(keep.cells[1]) if ($time > 0) $display("static1=%h", keep.cells[1]);

  initial begin
    shared = '{8'h00, 8'h22, 8'h00};
    record.cells = '{8'h00, 8'h22, 8'h00};
    record.tag = 8'h5a;
    keep.cells = '{8'h00, 8'h22, 8'h00};
    #1;
    load(shared);
    $readmemh("wake.hex", record.cells);
    keep;
    #1;
    $display("done ref=%h,%h,%h member=%h,%h,%h tag=%h static=%h,%h,%h",
             shared[0], shared[1], shared[2], record.cells[0], record.cells[1],
             record.cells[2], record.tag, keep.cells[0], keep.cells[1], keep.cells[2]);
    $finish;
  end
endmodule
