// SIM-006 A01: identity-handle elements (class objects, semaphores,
// mailboxes, processes, virtual interfaces) in fixed and resizable arrays.
// Copying an array copies handles, never the objects they designate
// (SV 7.4, 8.4, 9.7, 15.3, 15.4, 25.9).
`timescale 1ns/1ns
interface ifc;
  int v;
endinterface
module tb;
  class C;
    int v;
    function new(int x); v = x; endfunction
  endclass
  ifc i0();
  ifc i1();
  virtual ifc vifs[2:3];
  virtual ifc vq[$];
  C objs[3];
  C copy[3];
  C cq[$];
  semaphore sems[2];
  mailbox #(int) boxes[$];
  process procs[2];
  mailbox #(int) mb;
  int got;
  initial begin
    i0.v = 10;
    i1.v = 11;
    vifs[2] = i0;
    vifs[3] = i1;
    vq.push_back(vifs[3]);
    vq.push_back(vifs[2]);
    vq[0].v = 21;
    $display("vif %0d %0d %0d", i1.v, vifs[3].v, vq.size());
    foreach (objs[k]) objs[k] = new(k);
    copy = objs;
    copy[1].v = 50;
    cq = '{objs[2], objs[0]};
    cq[0].v = 60;
    $display("class %0d %0d %0d %0d", objs[1].v, objs[2].v, copy[2] == objs[2], cq.size());
    foreach (sems[k]) sems[k] = new(k + 1);
    $display("sem %0d %0d", sems[0].try_get(1), sems[1].try_get(2));
    mb = new();
    boxes.push_back(mb);
    boxes.push_back(boxes[0]);
    boxes[1].put(7);
    void'(boxes[0].try_get(got));
    $display("mailbox %0d %0d", got, boxes.size());
    fork
      begin procs[0] = process::self(); #5; end
      begin procs[1] = process::self(); #1; end
    join_none
    #2;
    $display("process %0d %0d", procs[0].status() == process::WAITING,
             procs[1].status() == process::FINISHED);
    procs[0].kill();
    $display("killed %0d", procs[0].status() == process::KILLED);
    $finish(0);
  end
endmodule
