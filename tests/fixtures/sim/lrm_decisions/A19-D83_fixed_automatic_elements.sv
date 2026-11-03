// Decision A19-D83: the elements of fixed-size array automatics of a frame
// (37.39, decisions A19-D76 and A19-D82).
// - A fixed-size array automatic (a local, an input, output or `ref`
//   formal, of packed, real or string elements, of one or more dimensions)
//   gives its elements: vpi_iterate(vpiReg) and vpi_iterate(vpiVarSelect)
//   in row-major order from the leftmost element, vpi_handle_by_index with
//   the declared index of a one-dimensional array (outside the range: NULL
//   and an error; a multidimensional array: NULL and an error) and
//   vpi_handle_by_multi_index with every declared index; vpiRange gives
//   its declared ranges, leftmost first.
// - An element is named after the variable and its declared indices
//   (vpiName arr[1], vpiFullName tb.t.arr[1], tb.t.m2[1][2]), is found by
//   that name while its frame is active, has vpiArrayMember and
//   vpiAutomatic TRUE and its array as vpiParent, reads its value and takes
//   vpi_put_value (vpiNoDelay) at once: the task sees it, a `ref` formal's
//   element is the actual's, and an output formal's element is copied out
//   at return.
// - vpiVarSelect also iterates the elements of a dynamic array, queue or
//   associative array, static or automatic.
// - Once the frame has ended, an element handle fails with an error (as
//   decision A19-D77) and the variable's name finds no object (NULL with
//   no error).
//
// IEEE 1800-2009 37.17 details 2 (SystemVerilog-1800-2009.txt L58741-58742):
// "The Boolean property vpiArrayMember shall be TRUE if the referenced
// variable is a member of an array variable"; 4 (L58744-58745): "For an
// array var, vpi_iterate(vpiRange, handle) shall return the set of array
// range declarations beginning with the leftmost unpacked range and
// iterating through the rightmost unpacked range"; 7 (L58757): "A var
// select is an element selected from an array var"; 9 (L58761): "vpiSize
// for a variable array shall return the number of variables in the array".
// 37.3.7 (L57527-57528): "An automatic variable that belongs to a frame
// (see 37.38) has the same object lifetime as that of the frame, which is
// alive from the point of the call that establishes the stack frame until
// the stack frame is destroyed." 37.39 detail 3 (L60162): "It shall be
// illegal to put a value with a delay on automatic variables."
// Reading an element handle after its frame has ended crashed one
// simulator; llg reports an error.
//
// Build A19-D83_fixed_automatic_elements.c as a VPI application (llg:
// static archive given to --dpi-lib; `task $d83(input int)` declared as a
// system task).
module tb;
  int dq[$];
  task automatic t(input int fa[3], ref int ra[2], output int fo[2]);
    int arr[4];
    logic [7:0] lar[1:3];
    int m2[2][3];
    real rd[2];
    string sa[2];
    int q[$];
    arr[0] = 10; arr[1] = 11; arr[2] = 12; arr[3] = 13;
    lar[1] = 8'h01; lar[2] = 8'hx2; lar[3] = 8'h03;
    m2[0][0] = 1; m2[0][1] = 2; m2[0][2] = 3; m2[1][0] = 4; m2[1][1] = 5; m2[1][2] = 6;
    rd[0] = 0.5; rd[1] = 1.5;
    sa[0] = "s0"; sa[1] = "s1";
    q.push_back(7);
    fo[0] = 21; fo[1] = 22;
    $d83(1);
    $display("in task: arr[1]=%0d lar[2]=%h m2[1][2]=%0d rd[1]=%0.2f sa[1]=%s fa[0]=%0d ra[1]=%0d fo[1]=%0d q[0]=%0d",
             arr[1], lar[2], m2[1][2], rd[1], sa[1], fa[0], ra[1], fo[1], q[0]);
  endtask
  initial begin
    int f3[3];
    int a2[2];
    int o2[2];
    f3[0] = 3; f3[1] = 4; f3[2] = 5;
    a2[0] = 1; a2[1] = 2;
    dq.push_back(30); dq.push_back(31);
    t(f3, a2, o2);
    $display("after: a2=%0d,%0d o2=%0d,%0d f3[0]=%0d", a2[0], a2[1], o2[0], o2[1], f3[0]);
    $d83(2);
    $finish;
  end
endmodule
