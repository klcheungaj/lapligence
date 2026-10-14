// SIM-027 A03: the memory_name argument of a memory file task is evaluated
// once (decision S27-D13). Every selector below calls next(), which counts
// its calls; each task call must count exactly one, for a load (whose root
// is read and stored) and for a dump.
module tb;
  typedef struct { logic [7:0] mem [0:3]; logic [3:0] tag; } record_t;
  int calls;
  record_t records [0:2];
  logic [7:0] back [0:3];

  function automatic int next();
    calls++;
    return calls - 1;
  endfunction

  task automatic show(string label, logic [7:0] a, b, c, d);
    $display("%s calls=%0d %h %h %h %h", label, calls, a, b, c, d);
  endtask

  task automatic load_ref(ref logic [7:0] cells [0:3]);
    $readmemh("words.mem", cells);
  endtask

  task automatic automatic_forms;
    record_t locals [0:2];
    logic [7:0] grid [0:1][0:3];
    logic [7:0] shared [0:1][0:3];
    calls = 0;
    $readmemh("words.mem", locals[next()].mem);
    show("automatic member load", locals[0].mem[0], locals[0].mem[1],
         locals[0].mem[2], locals[0].mem[3]);
    $display("untouched %h %h", locals[1].mem[0], locals[0].tag);
    calls = 0;
    $writememh("dump.mem", locals[next()].mem);
    $readmemh("dump.mem", back);
    show("automatic member dump", back[0], back[1], back[2], back[3]);
    calls = 1;
    $readmemh("words.mem", grid[next()]);
    show("automatic grid row", grid[1][0], grid[1][1], grid[1][2], grid[1][3]);
    calls = 0;
    fork
      $readmemh("words.mem", shared[next()]);
    join
    show("fork-shared row", shared[0][0], shared[0][1], shared[0][2], shared[0][3]);
    calls = 1;
    load_ref(locals[next()].mem);
    show("ref formal", locals[1].mem[0], locals[1].mem[1], locals[1].mem[2],
         locals[1].mem[3]);
  endtask

  initial begin
    int fd;
    fd = $fopen("words.mem", "w");
    $fwrite(fd, "11\n22\n33\n44\n");
    $fclose(fd);
    calls = 2;
    $readmemh("words.mem", records[next()].mem);
    show("module member load", records[2].mem[0], records[2].mem[1],
         records[2].mem[2], records[2].mem[3]);
    calls = 2;
    $writememh("dump.mem", records[next()].mem, 3, 0);
    $readmemh("dump.mem", back);
    show("module member dump", back[0], back[1], back[2], back[3]);
    calls = 0;
    $readmemh("words.mem", back, next() + 1);
    show("start bound", back[0], back[1], back[2], back[3]);
    automatic_forms();
    $finish;
  end
endmodule
