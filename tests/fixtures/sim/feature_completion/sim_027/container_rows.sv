// SIM-027 A03: dynamic arrays and queues whose elements are unpacked arrays
// as memory_name (IEEE 1800-2009 21.4: "a partially indexed multidimensional
// unpacked array that resolves to a lesser dimensioned unpacked array"). One
// element (`da[k]`, also with a further index or slice inside it), the whole
// container or a slice of it is addressed in place, at the current size and
// never resized (21.4.1). An unknown or missing element and a slice outside
// the current size are errors that change nothing (S27-D19).
module tb;
  class holder;
    byte rows [][3];
  endclass

  int da [][4];
  logic [7:0] dq [$][1:0];
  logic [7:0] grid [][2][2];
  int empty [][2];
  logic [7:0] back [0:3];
  int k;
  int j;
  int calls;
  integer unknown;

  function automatic int pick(int v);
    calls++;
    return v;
  endfunction

  initial begin
    int fd;
    holder h;
    fd = $fopen("four.mem", "w");
    $fwrite(fd, "11\n22\n33\n44\n");
    $fclose(fd);

    // One element selected at run time, evaluated once.
    da = new[3];
    calls = 0;
    $readmemh("four.mem", da[pick(1)]);
    $display("row %p calls %0d", da, calls);
    // A variable slice inside an element, and a start/finish range.
    j = 1;
    $readmemh("four.mem", da[2][j+:2], j + 1, j);
    $display("inner %p", da);
    // The whole container, then a variable slice of its elements.
    $readmemh("four.mem", da[0+:1]);
    k = 1;
    $readmemh("four.mem", da[k+:2], 2, 2);
    $display("outer %p size %0d", da, da.size());
    // A dump of one element, read back.
    $writememh("dump.mem", da[k]);
    $readmemh("dump.mem", back);
    $display("dump %h %h %h %h", back[0], back[1], back[2], back[3]);

    // A queue of descending two-element arrays: a slice ending at `$`, and
    // the last element.
    dq.push_back('{8'h01, 8'h02});
    dq.push_back('{8'h03, 8'h04});
    dq.push_back('{8'h05, 8'h06});
    $readmemh("four.mem", dq[1:$]);
    $display("queue %p", dq);
    $readmemh("four.mem", dq[$], 0, 0);
    $display("queue last %p size %0d", dq, dq.size());

    // Two-dimensional elements: the whole container, a row of an element.
    grid = new[1];
    $readmemh("four.mem", grid);
    $display("grid %p", grid);
    $readmemh("four.mem", grid[0][1], 1, 0);
    $display("grid row %p", grid);

    // A class property reached through a handle.
    h = new;
    h.rows = new[2];
    $readmemh("four.mem", h.rows[k], 0, 2);
    $display("handle %p", h.rows);

    // Errors and an empty container: nothing changes, no file is written.
    k = 3;
    $readmemh("four.mem", da[k]);
    unknown = 'x;
    $readmemh("four.mem", da[unknown]);
    $readmemh("four.mem", da[k-1+:2]);
    $writememh("dump.mem", da[k]);
    $readmemh("four.mem", empty);
    $display("kept %p", da);
    $readmemh("dump.mem", back);
    $display("kept dump %h %h %h %h", back[0], back[1], back[2], back[3]);
    $finish;
  end
endmodule
