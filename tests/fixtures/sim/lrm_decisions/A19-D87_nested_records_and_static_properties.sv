// Decision A19-D87: elements of nested containers and of arrays of more
// dimensions or of records, and static class properties.
// - An array whose elements hold queues, dynamic or associative arrays is
//   flattened: its elements are the innermost ones (vpiSize counts them,
//   vpiReg iterates them in index order), each named by all its indices
//   (qq[1][0]) with the whole array as vpiParent. vpi_handle_by_index and
//   vpi_handle_by_multi_index give NULL and an error, as does a name with
//   fewer indices (no subarray object, decision A20-D23).
// - vpi_iterate(vpiIndex) of such an element, or of an element of a
//   fixed-size array of more dimensions, gives its indices innermost first
//   (decision A19-D28); vpi_handle(vpiIndex) gives NULL and an error.
// - An element of an array of unpacked structures is a vpiStructVar whose
//   vpiSize is its number of members; vpiMember lists them, each named by
//   its member name, with full name tb.rq[0].a and the element as
//   vpiParent.
// - An element is automatic with its array: FALSE for an array of a
//   module or a static class property, TRUE for an instance property.
// - A static class property is listed by vpiVariables (an event by
//   vpiNamedEvent) of every object of its class or of a derived class and of
//   the class definition that declares it; its vpiFullName is the class
//   definition's full name, :: and its name (tb.C::si) however it is reached;
//   vpi_handle_by_name finds it under that name or through a class
//   variable (tb.p.si); vpiAutomatic is FALSE; its value is read and written.
//
// IEEE 1800-2009 37.17 detail 25 (SystemVerilog-1800-2009.txt
// L58825-58837): "The static data member of a class, referenced either via
// a class var or a class defn, has the vpiFullName property. It shall
// return a full name string representing the hierarchical path of the
// static variable through "class defn". [...] The vpiFullName for p.Id is
// "top.Packet::Id"."; 37.17 detail 4 (L58744-58750) and detail 9
// (L58760-58764); 37.17 detail 5 (L58751-58753): vpiIndex of a var select
// in a multidimensional array "starting with the index for the var select
// and working outward"; 37.3.7 (lifetime). Industry-standard behaviour
// names a static property after the path that reached it (tb.p.si), finds
// none under tb.C::si, gives an element of a structure array vpiSize 1,
// lists indices outermost first and gives vpi_handle_by_multi_index of a
// nested array NULL without an error; llg follows the text where it
// speaks and keeps the error on a refused selection.
//
// Build A19-D87_nested_records_and_static_properties.c as a VPI
// application (llg: static archive given to --dpi-lib; `task $d87()`
// declared as a system task).
module tb;
  typedef struct {
    int a;
    string s;
  } rec_t;
  class C;
    static int si = 4;
    static int sa[2];
    static event se;
    int da[];
  endclass
  class D extends C;
    static string ds = "d";
  endclass
  int qq[$][$];
  string s2[2][3];
  rec_t rq[$];
  D p;
  int woke;
  initial @(C::se) woke = 1;
  initial begin
    #1;
    qq.push_back('{1, 2});
    qq.push_back('{3});
    s2[1][2] = "x";
    rq.push_back('{5, "five"});
    p = new;
    p.da = new[1];
    C::sa[1] = 7;
    $d87();
    #1;
    $display("qq[1][0]=%0d rq[0].a=%0d si=%0d sa[1]=%0d ds=%s woke=%0d", qq[1][0], rq[0].a,
             C::si, C::sa[1], D::ds, woke);
  end
endmodule
