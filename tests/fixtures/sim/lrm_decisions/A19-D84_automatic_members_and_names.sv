// Decision A19-D84: members of structure automatics and automatics by
// name (37.39, decisions A19-D76, A19-D82 and A19-D83).
// - An unpacked structure automatic (a local, an input or output formal)
//   and a packed structure automatic or an element of a packed structure
//   array give their members: vpi_iterate(vpiMember) in declaration order,
//   each named after its variable (vpiName a, vpiFullName tb.t.rl.a,
//   tb.t.pa[1].h), with vpiParent the variable or element holding it,
//   vpiStructUnionMember TRUE, vpiAutomatic TRUE, its value and
//   vpi_put_value (vpiNoDelay), which the task sees at once and an output
//   formal copies out at return. vpi_handle_by_name with the structure as
//   scope finds a member by its name. The unpacked structure itself has no
//   value.
// - vpi_handle_by_name finds an automatic of a live frame by its full name
//   (tb.rt.loc), an element or member below it (tb.t.pa[1].h): in the
//   innermost active frame of that task or function on the running thread,
//   so a recursive task names its innermost activation. With no live frame
//   of the subprogram the name finds no object: NULL with no error.
//
// IEEE 1800-2009 37.24 detail 1 (SystemVerilog-1800-2009.txt L59271-59272):
// "vpi_get_value()/vpi_put_value() cannot be used to access values of
// entire unpacked structures and unpacked unions." 37.17 detail 17
// (L58787-58788): "When the vpiStructUnionMember property is TRUE, it
// indicates that the variable is a member of a parent struct or union
// variable." 37.3.7 (L57527-57528, L57538-57539): "An automatic variable
// that belongs to a frame (see 37.38) has the same object lifetime as that
// of the frame, which is alive from the point of the call that establishes
// the stack frame until the stack frame is destroyed"; vpiAutomatic "when
// true, it means the object is non-static". 37.39 detail 4 (L60163): "There
// is at most only one active frame at any time in a given thread." 38.21
// (L63378-63379): vpi_handle_by_name "can be applied to all objects with a
// fullname property". Some simulators report vpiAutomatic FALSE for the
// members of an automatic structure and return a handle without a value
// for the name of an automatic with no live frame; llg follows 37.3.7.
//
// Build A19-D84_automatic_members_and_names.c as a VPI application (llg:
// static archive given to --dpi-lib; `task $d84(input int)` declared as a
// system task).
module tb;
  typedef struct { int a; logic [7:0] b; } rec_t;
  typedef struct packed { logic [3:0] h; logic [3:0] l; } pk_t;
  int g;
  task automatic t(input rec_t ri, output rec_t ro);
    rec_t rl;
    pk_t pl;
    pk_t pa[2];
    rl.a = 6;
    rl.b = 8'h3c;
    pl = 8'hc3;
    pa[0] = 8'h12;
    pa[1] = 8'h34;
    ro.a = 11;
    ro.b = 8'h0c;
    $d84(1);
    $display("in task: rl.a=%0d ri.a=%0d ro.a=%0d pl=%h pa[1]=%h", rl.a, ri.a, ro.a, pl, pa[1]);
  endtask
  task automatic rt(input int n);
    int loc;
    loc = n * 10;
    if (n > 0) rt(n - 1);
    else $d84(2);
    $d84(3);
    g = g + loc;
  endtask
  initial begin
    rec_t r;
    rec_t o;
    r.a = 5;
    r.b = 8'h11;
    $d84(0);
    t(r, o);
    $display("after: o.a=%0d o.b=%0d", o.a, o.b);
    g = 0;
    rt(2);
    $display("after rt: g=%0d", g);
    $d84(0);
    $finish;
  end
endmodule
