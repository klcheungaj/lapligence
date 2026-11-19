// UVB-D1: an unpacked array of derived-class handles assigned to an unpacked
// array of base-class handles copies each handle; the frontend warns.
//
// IEEE 1800-2009 7.6 (SystemVerilog-1800-2009.txt L8422-8424):
//   "A fixed-size unpacked array, dynamic array or queue, or a slice of such an
//    array, shall be assignment compatible with any other such array or slice
//    if all the following conditions are satisfied:
//    -  The element types of source and target shall be equivalent."
// 6.22.2 (L7106): "a) If two types match, they are equivalent." 6.22.1 d)
// (L7051-7052) makes a class type match only itself. 7.7 (L8517-8518)
// applies the 7.6 rule to an array argument passed by value. 8.15 (L10196)
// allows a subclass handle in a base class variable, one handle at a time.
//
// Decision (user decision 2026-11-19, industry-standard behaviour): the
// assignment, an argument, a return value and an output copy-out of such an
// array are accepted with a located warning citing 7.6 (`-Werror` makes it
// an error) for fixed-size, dynamic and queue arrays among themselves, a
// slice, associative arrays of the same index type, nested fixed dimensions
// and any depth of derivation. Each element is copied as a handle: the
// target holds the same objects, and later changes of the source do not
// reach it. The reverse direction stays an error
// (UVB-D1_base_handle_array_to_derived_rejected.sv).
class base;
  int id;
  function new(int id); this.id = id; endfunction
endclass
class derived extends base;
  function new(int id); super.new(id); endfunction
endclass
class grand extends derived;
  function new(int id); super.new(id); endfunction
endclass
typedef base base_dyn[];
module tb;
  derived dd[];
  derived dq[$];
  derived df[3];
  derived da[int];
  derived ds[string];
  derived d2[2][2];
  grand gd[];
  base bd[];
  base bq[$];
  base bf[3];
  base ba[int];
  base bs[string];
  base b2[2][2];
  base bo[];
  function automatic base_dyn ret_dyn();
    return dd;
  endfunction
  task automatic t_out(output derived o[]);
    o = dd;
  endtask
  task automatic t_in(input base i[]);
    $display("t_in size %0d first %0d", i.size(), i[0].id);
  endtask
  initial begin
    dd = new[3];
    foreach (dd[i]) dd[i] = new(10 + i);
    foreach (df[i]) df[i] = new(20 + i);
    dq.push_back(dd[0]); dq.push_back(dd[2]);
    da[5] = dd[1]; da[-1] = dd[0];
    ds["x"] = dd[2];
    foreach (d2[i, j]) d2[i][j] = new(30 + 2 * i + j);
    gd = new[2]; gd[0] = new(40); gd[1] = new(41);
    bd = dd;
    $display("dyn->dyn %0d %0d %0d same=%0d", bd.size(), bd[0].id, bd[2].id, bd[0] == dd[0]);
    dd[0] = new(99);
    $display("after src change bd[0]=%0d dd[0]=%0d", bd[0].id, dd[0].id);
    bq = dd;
    $display("dyn->queue %0d %0d", bq.size(), bq[0].id);
    bq = df;
    $display("fixed->queue %0d %0d", bq.size(), bq[2].id);
    bq = dq;
    $display("queue->queue %0d %0d", bq.size(), bq[1].id);
    bd = dq;
    $display("queue->dyn %0d %0d", bd.size(), bd[1].id);
    bf = dd;
    $display("dyn->fixed %0d %0d", bf[0].id, bf[2].id);
    bf = df;
    $display("fixed->fixed %0d", bf[1].id);
    ba = da;
    $display("assoc->assoc %0d %0d %0d", ba.num(), ba[5].id, ba[-1].id);
    bs = ds;
    $display("assoc str %0d %0d", bs.num(), bs["x"].id);
    b2 = d2;
    $display("2d %0d %0d", b2[0][1].id, b2[1][0].id);
    bd = gd;
    $display("grand->base %0d %0d", bd.size(), bd[1].id);
    bd = ret_dyn();
    $display("return %0d %0d", bd.size(), bd[0].id);
    t_out(bo);
    $display("output formal %0d %0d", bo.size(), bo[1].id);
    t_in(dd);
    t_in(df);
    t_in(dq);
    bd = df[1:2];
    $display("slice %0d %0d", bd.size(), bd[0].id);
    $finish;
  end
endmodule
