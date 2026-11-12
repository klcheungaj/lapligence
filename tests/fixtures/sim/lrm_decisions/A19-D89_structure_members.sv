// Decision A19-D89: unpacked, string, handle and container members of
// structures, and the member forms llg refuses.
// - vpiMember lists the members in declaration order.
// - vpiStructUnionMember is TRUE for the member itself and FALSE for an
//   element of an array member.
// - An element of an array member takes cbValueChange, and a
//   vpi_put_value to it wakes a process waiting on it.
// - A structure property of a class object lists its members.
// - An unpacked array member of strings or structures kept one leaf per
//   cell, a select of a member of a dynamic element and a member of a
//   structure property kept as one packed value give NULL with an error.
//
// IEEE 1800-2009 37.24 (SystemVerilog-1800-2009.txt L59260-59275) gives no
// member order; 37.17 detail 17 (L58778-58779) and 37.18 detail 5
// (L58959-58961): "This property shall be FALSE for all sub-elements ...";
// 38.36.1 makes cbValueChange apply to variables, and an element of an
// array member is one; a put with vpiNoDelay changes the variable as an
// assignment would. Industry-standard behaviour lists scalar members
// first, gives an element of an array member vpiStructUnionMember TRUE,
// refuses a value-change callback on the element of an array member of
// some structures (NULL without an error), does not wake a process
// waiting on it after a put and serves the refused forms; llg follows the
// text for the first four.
//
// Build A19-D89_structure_members.c as a VPI application (llg: static
// archive given to --dpi-lib; `task $d89(input int)` declared as a system
// task).
`timescale 1ns/1ns
typedef struct {
  int k;
  logic [7:0] b;
} pe_t;
typedef struct {
  int fa[2];
  string s;
  int k;
} pr_t;
typedef struct {
  int arr[2];
  string sa[2];
  int k;
} rec_t;
class C;
  pr_t pr;
  pe_t pe;
  function new();
    pr.k = 3;
    pe.k = 4;
  endfunction
endclass
module tb;
  rec_t r;
  pr_t ea[2];
  C c;
  initial begin
    c = new;
    r.k = 1;
    r.sa[0] = "a";
    ea[1].k = 5;
    $d89(1);
    #1 $d89(2);
    #1 $display("r.arr[1]=%0d c.pr.k=%0d c.pe.k=%0d ea[1].k=%0d", r.arr[1], c.pr.k, c.pe.k,
                ea[1].k);
    $finish;
  end
  always @(r.arr[1]) $display("%0t: woke on r.arr[1]=%0d", $time, r.arr[1]);
endmodule
