// SIM-027 A03: slices with a variable position (IEEE 1800-2009 7.4.6) and
// queue slices whose bounds are not constant or use `$` (7.10.1) as
// memory_name (21.4, 21.5). The position and each bound are evaluated once
// when the task runs; a slice that is not inside the array (declared range
// or current size), has an unknown position or reversed queue bounds is an
// error that loads nothing and writes no file (decisions S27-D19, S27-D20).
module tb;
  class holder;
    logic [7:0] cells [0:5];
    logic [7:0] q [$];
    task load(int p);
      $readmemh("two.mem", this.cells[p+:2]);
      $readmemh("two.mem", this.q[p:$]);
    endtask
  endclass

  logic [7:0] up [0:7];
  logic [7:0] down [7:0];
  logic [7:0] grid [0:2][0:3];
  logic [7:0] dyn [];
  logic [7:0] fifo [$];
  logic [7:0] back [0:2];
  int i;
  int calls;
  integer unknown;

  function automatic int pick(int v);
    calls++;
    return v;
  endfunction

  task automatic local_image(int p);
    logic [7:0] loc [0:5];
    $readmemh("three.mem", loc[p+:3]);
    $display("local %h %h %h %h %h %h", loc[0], loc[1], loc[2], loc[3], loc[4], loc[5]);
    $writememh("dump.mem", loc[p+1-:3]);
  endtask

  task show_up;
    $display("up %h %h %h %h %h %h %h %h", up[0], up[1], up[2], up[3], up[4], up[5], up[6],
             up[7]);
  endtask

  initial begin
    int fd;
    holder h;
    fd = $fopen("two.mem", "w");
    $fwrite(fd, "11\n22\n");
    $fclose(fd);
    fd = $fopen("three.mem", "w");
    $fwrite(fd, "31\n32\n33\n");
    $fclose(fd);

    // Fixed module arrays, ascending and descending declarations.
    i = 2;
    $readmemh("three.mem", up[i+:3]);
    $readmemh("two.mem", up[i+4-:2]);
    show_up();
    $readmemh("three.mem", down[i+:3]);
    $display("down %h %h %h %h %h %h %h %h", down[0], down[1], down[2], down[3], down[4],
             down[5], down[6], down[7]);
    $readmemh("two.mem", down[i+:3], 4, 3);
    $display("down range %h %h %h", down[2], down[3], down[4]);

    // A runtime row and a variable slice of a two-dimensional memory.
    calls = 0;
    $readmemh("two.mem", grid[pick(1)][pick(1)+:2]);
    $display("grid %h %h %h %h calls %0d", grid[1][0], grid[1][1], grid[1][2], grid[1][3],
             calls);

    // A dump of a variable slice, read back.
    $writememh("dump.mem", up[i+3-:3]);
    $readmemh("dump.mem", back);
    $display("dump %h %h %h", back[0], back[1], back[2]);

    // Class properties: `this.` inside a method, and an explicit handle.
    h = new;
    h.q = '{8'h01, 8'h02, 8'h03, 8'h04};
    h.load(1);
    $display("this %h %h %h %h q %h %h %h %h size %0d", h.cells[0], h.cells[1], h.cells[2],
             h.cells[3], h.q[0], h.q[1], h.q[2], h.q[3], h.q.size());
    i = 3;
    $readmemh("two.mem", h.cells[i+:2]);
    $display("handle %h %h %h", h.cells[2], h.cells[3], h.cells[4]);

    // A dynamic array and a queue, in place, sizes unchanged.
    dyn = new[5];
    $readmemh("three.mem", dyn[i-:3]);
    $display("dyn %h %h %h %h %h size %0d", dyn[0], dyn[1], dyn[2], dyn[3], dyn[4], dyn.size());
    fifo = '{8'h01, 8'h02, 8'h03, 8'h04, 8'h05};
    $readmemh("two.mem", fifo[i:$]);
    $display("fifo %h %h %h %h %h size %0d", fifo[0], fifo[1], fifo[2], fifo[3], fifo[4],
             fifo.size());
    $readmemh("three.mem", fifo[$-2:$]);
    $display("fifo end %h %h %h %h %h", fifo[0], fifo[1], fifo[2], fifo[3], fifo[4]);
    $writememh("dump.mem", fifo[i-2:$-1]);
    $readmemh("dump.mem", back);
    $display("fifo dump %h %h %h", back[0], back[1], back[2]);

    // An automatic memory (a packed image).
    local_image(2);
    $readmemh("dump.mem", back);
    $display("local dump %h %h %h", back[0], back[1], back[2]);

    // Slices not inside the array, or with an unknown position or reversed
    // queue bounds: errors that load nothing and write no file.
    i = 6;
    $readmemh("two.mem", up[i+:3]);
    i = 1;
    $readmemh("two.mem", down[i-:3]);
    unknown = 'x;
    $readmemh("two.mem", up[unknown+:2]);
    i = 4;
    $readmemh("two.mem", dyn[i+:2]);
    $readmemh("two.mem", fifo[3:1]);
    $writememh("dump.mem", fifo[i+1:$]);
    $writememh("dump.mem", up[i+4-:2]);
    show_up();
    $display("down %h %h %h dyn %h %h fifo %h %h %h %h %h", down[0], down[1], down[2], dyn[3],
             dyn[4], fifo[0], fifo[1], fifo[2], fifo[3], fifo[4]);
    $readmemh("dump.mem", back);
    $display("kept dump %h %h %h", back[0], back[1], back[2]);
    $finish;
  end
endmodule
