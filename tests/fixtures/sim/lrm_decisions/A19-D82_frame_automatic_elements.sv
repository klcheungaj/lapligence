// Decision A19-D82: the automatics of a frame (37.39 detail 1, decision
// A19-D76) also serve the following.
// - A dynamic array, queue or associative array automatic gives its
//   elements: vpi_iterate(vpiReg) in index or key order and
//   vpi_handle_by_index, each named after the variable and its index
//   (vpiName q[1], vpiFullName tb.t.q[1]); an element reads its value and
//   takes vpi_put_value (vpiNoDelay), which the task then sees. An element
//   handle fails with an error once its frame has ended, as the automatic's
//   own handle does (decision A19-D77).
// - A string or real `ref` formal reads and writes its actual.
// - An unpacked structure automatic reports its number of fields as
//   vpiSize and has no value.
//
// IEEE 1800-2009 37.39 (SystemVerilog-1800-2009.txt L60160): "1) The
// iteration on vpiAutomatics shall return all automatic variables declared
// in the frame". 37.17 detail 9 (L58763): "For unpacked structures and
// unions, the size returned indicates the number of fields in the
// structure or union"; detail 11 (L58766-58767): "Struct var and union
// var variables for which the vpiVector property is FALSE do not have a
// value property". The array diagram of 37.17 reaches the elements of an
// array var through vpiReg and vpi_handle_by_index. 13.5.2: a `ref`
// argument is passed by reference, so its value is the actual's. Reading
// an element handle after its frame has ended crashed one simulator; llg
// reports an error, as for the automatic's own handle (A19-D77).
//
// Build A19-D82_frame_automatic_elements.c as a VPI application (llg:
// static archive given to --dpi-lib; `task $d82(input int)` declared as a
// system task).
module tb;
  typedef struct { int a; logic [7:0] b; } rec_t;
  int g;
  task automatic t(ref string rs, ref real rr, input rec_t ri);
    int q[$];
    logic [7:0] d[];
    int aa[string];
    q.push_back(100);
    q.push_back(-200);
    d = new[3];
    d[0] = 8'h05;
    d[1] = 8'hx5;
    d[2] = 8'h07;
    aa["k"] = 9;
    aa["j"] = 8;
    $d82(1);
    $display("in task: q[1]=%0d d[2]=%0d rs=%s rr=%0.2f aa[j]=%0d", q[1], d[2], rs, rr, aa["j"]);
    g = q[0] + ri.a;
  endtask
  initial begin
    string s;
    real r;
    rec_t x;
    s = "str";
    r = 1.5;
    x.a = 5;
    x.b = 8'h3c;
    t(s, r, x);
    $display("after: s=%s r=%0.2f g=%0d", s, r, g);
    $d82(2);
    $finish;
  end
endmodule
