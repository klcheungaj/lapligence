// IEEE 1800-2009 11.4.14.3, 11.4.14.4, 13.5: an output formal copies out
// into a streaming concatenation whose `with` range is runtime-valued. The
// formal's value is unpacked into the selected elements in storage order,
// consuming the leftmost bits; the range is selected when the call starts,
// and a disabled call copies nothing out.
module tb;
  timeunit 1ns;
  timeprecision 1ns;
  typedef struct { bit [3:0] a; logic [3:0] b; } mix_t;
  logic [7:0] arr [0:7];
  logic [7:0] dsc [7:0];
  mix_t cells [0:3];
  int n;
  event ev;
  task automatic get(output logic [31:0] v);
    v = 32'h11223344;
  endtask
  task automatic get_bump(output logic [23:0] v);
    n = n + 3;
    v = 24'hA1B2C3;
  endtask
  task static sget(output logic [15:0] v);
    v = 16'hBEEF;
  endtask
  task automatic eget(output logic [15:0] v, input event e);
    -> e;
    v = 16'hCAFE;
  endtask
  task automatic tget(output logic [15:0] v);
    #2 v = 16'h1357;
  endtask
  function automatic void fget(output logic [15:0] v);
    v = 16'h2468;
  endfunction
  task automatic mixget(output logic [15:0] v);
    v = 16'hxA_5z;
  endtask
  task automatic local_case;
    logic [7:0] loc [1:4];
    int k;
    foreach (loc[j]) loc[j] = 8'h00;
    k = 2;
    get({>>{loc with [k +: 3]}});
    $display("local %h %h %h %h", loc[1], loc[2], loc[3], loc[4]);
  endtask
  initial begin
    foreach (arr[k]) arr[k] = 8'h00;
    foreach (dsc[k]) dsc[k] = 8'h00;
    foreach (cells[k]) cells[k] = '{4'h0, 4'h0};
    n = 2;
    get({>>{arr with [n +: 4]}});
    $display("plus %h %h %h %h %h %h", arr[1], arr[2], arr[3], arr[4], arr[5], arr[6]);
    n = 1;
    get_bump({>>{arr with [n +: 3]}});
    $display("frozen n=%0d %h %h %h %h", n, arr[1], arr[2], arr[3], arr[4]);
    n = 5;
    get({<<8{dsc with [n -: 4]}});
    $display("reversed %h %h %h %h %h %h", dsc[1], dsc[2], dsc[3], dsc[4], dsc[5], dsc[6]);
    n = 0;
    sget({>>{arr with [n +: 2]}});
    $display("static %h %h", arr[0], arr[1]);
    eget({>>{arr with [n +: 2]}}, ev);
    $display("expanded %h %h", arr[0], arr[1]);
    n = 3;
    tget({>>{arr with [n +: 2]}});
    $display("timed %0t %h %h", $time, arr[3], arr[4]);
    fork
      tget({>>{arr with [n +: 2]}});
      #1 disable tget;
    join
    $display("disabled %h %h", arr[3], arr[4]);
    fget({>>{arr with [n -: 2]}});
    $display("function %h %h", arr[2], arr[3]);
    n = 1;
    mixget({>>{cells with [n +: 2]}});
    $display("mixed %h %h | %h %h | %h %h", cells[0].a, cells[0].b, cells[1].a, cells[1].b,
             cells[2].a, cells[2].b);
    local_case();
    $finish(0);
  end
endmodule
