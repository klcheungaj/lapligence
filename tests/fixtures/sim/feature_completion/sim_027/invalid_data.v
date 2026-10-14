// SIM-027 A01/A03: erroneous and out-of-range memory files never write
// outside the selected memory (IEEE 1364-2001 17.2.8; IEEE 1800-2009 21.4).
// Neighbouring memories, the other rows of a selected row and the cells of
// an automatic memory around a slice keep their values. Expected values are
// derived by hand from the data files.
module tb;
  reg [7:0] below [0:3];
  reg [7:0] target [0:3];
  reg [7:0] above [0:3];
  integer i;

  task fill;
    begin
      for (i = 0; i < 4; i = i + 1) begin
        below[i] = 8'hb0 + i;
        target[i] = 8'hee;
        above[i] = 8'ha0 + i;
      end
    end
  endtask

  task show;
    input [8*16-1:0] label;
    begin
      $display("%0s target=%h,%h,%h,%h guards=%h%h%h%h,%h%h%h%h", label,
               target[0], target[1], target[2], target[3],
               below[0], below[1], below[2], below[3],
               above[0], above[1], above[2], above[3]);
    end
  endtask

  task automatic automatic_neighbours;
    reg [7:0] cells [0:5];
    integer k;
    begin
      for (k = 0; k < 6; k = k + 1) cells[k] = 8'hc0 + k;
      $readmemh("long.hex", cells, 2, 3);
      $display("automatic long=%h,%h,%h,%h,%h,%h", cells[0], cells[1], cells[2], cells[3], cells[4], cells[5]);
      for (k = 0; k < 6; k = k + 1) cells[k] = 8'hc0 + k;
      $readmemh("outside_task.hex", cells, 1, 4);
      $display("automatic outside=%h,%h,%h,%h,%h,%h", cells[0], cells[1], cells[2], cells[3], cells[4], cells[5]);
    end
  endtask

  initial begin
    fill;
    $readmemh("missing.hex", target);
    show("missing");
    fill;
    $readmemh("bad_digit.hex", target);
    show("bad_digit");
    fill;
    $readmemh("outside_task.hex", target, 0, 3);
    show("outside_task");
    fill;
    $readmemh("outside_memory.hex", target);
    show("outside_memory");
    fill;
    $readmemh("unknown_address.hex", target);
    show("unknown_address");
    fill;
    $readmemh("long.hex", target);
    show("long");
    fill;
    $readmemh("short.hex", target);
    show("short");
    fill;
    $readmemh("short.hex", target, 1, 9);
    show("finish_outside");
    fill;
    $readmemh("short.hex", target, -1);
    show("start_outside");
    automatic_neighbours;
    $finish;
  end
endmodule
