// Decision A19-D44: frames and threads. See docs/lrm_decisions.md for the
// decision and the expected output.
//
// IEEE 1800-2009 37.39 detail 1 (SystemVerilog-1800-2009.txt L60160):
// "Frames correspond to the set of automatic variables declared in a given
// task or function."; detail 4 (L60163-L60165): "There is at most only one
// active frame at any time in a given thread. To get a handle to the
// currently active frame, use vpi_handle(vpiFrame, NULL). The frame to stmt
// transition shall return the currently active statement within the
// frame." 37.40 detail 1 (L60195-L60197): "A thread is a SystemVerilog
// process such as an always procedure or a branch of a fork construct. As a
// thread works its way down a call chain of tasks and/or functions, a new
// frame is activated as each new task or function is entered." 38.36.1
// (L64658-L64671): cbStartOfFrame, cbEndOfFrame, cbStartOfThread,
// cbEndOfThread and cbEnterThread; after cbEndOfFrame and cbEndOfThread
// "any handles to this frame" / "this thread ... should be considered
// invalid". 37.3.8 (L57568): a handle "remains valid only as long as the
// object exists". 37.25 detail 1 (L59303-L59304): vpiWaitingProcesses of a
// named event returns its waiting processes "identified by their thread".
//
// Text decides (another simulator differs): only task and function calls
// activate frames, so an initial procedure or a named block has none; only
// the innermost frame of a thread is active; handles to ended frames and
// threads fail with an error. Text silent, another simulator followed: a
// thread is active while it exists and names no origin statement;
// cbStartOfFrame is placed on a task or function, cbStartOfThread on no
// object, cbEndOfThread on a thread.
module tb;
  event ev;
  int r, hits;
  task automatic t1(input int a);
    begin : blk
      int x;
      x = a;
      r = x;
      $d44(1);
    end
  endtask
  task t2;
    $d44(2);
  endtask
  function automatic int fact(int n);
    if (n <= 1) begin
      $d44(3);
      return 1;
    end
    return n * fact(n - 1);
  endfunction
  initial begin
    $d44(0);
    t1(4);
    t2;
    r = fact(2);
    fork
      #1 $d44(4);
    join
    -> ev;
    #1 $d44(5);
    $display("r=%0d hits=%0d", r, hits);
    $finish;
  end
  always @(ev) hits = hits + 1;
endmodule
