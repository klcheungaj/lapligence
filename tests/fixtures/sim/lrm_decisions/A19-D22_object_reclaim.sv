// Decision A19-D22: class object lifetime through VPI. A handle to a class
// object, its properties or the elements of their arrays does not keep the
// object alive: when the collector reclaims it, the cbReclaimObj and
// cbEndOfObject callbacks run first (no time), then every handle into the
// object is released and later use fails with vpiError. vpiObjId numbers
// objects from 1 in the order VPI first sees them; ids are not reused. A
// callback on a class typespec matches objects created with exactly that
// class, not objects of derived classes. During cbReclaimObj the object's
// properties read, but navigation to other objects, vpi_put_value and
// callback registration on the object fail with vpiError; the handle passed
// to the callback is released when the callbacks return.
//
// IEEE 1800-2009 38.36.1 (SystemVerilog-1800-2009.txt L64653-L64656):
// cbReclaimObj runs "Before the class object has been reclaimed by the
// automatic memory management, when it has been marked as no longer being
// used. When control is returned from this callback, any handles to this
// class object, its properties or their subelements, and any associated
// callbacks should be considered invalid."; L64694-L64695: "With a class
// typespec, any class object of that type shall generate a callback.";
// L64777-L64778: "All VPI properties of the class obj are accessible. Using
// this handle as a reference for purposes of navigation or registering
// callbacks is undefined."; 37.29 detail 1 (L59610-L59612): vpiObjId "is a
// property of a live object and guaranteed to be unique with respect to all
// other dynamic objects that support this property for as long as the
// object is alive. After the object is destroyed by garbage collection, its
// particular vpiObjId value may be reused." Whether a handle pins its
// object, what "of that type" means for derived classes, the numbering and
// the refusals are llg choices.
//
// Run with LLG_GC_STRESS=1 and LLG_GC_VERIFY=1. Build
// A19-D22_object_reclaim.c as a VPI application (llg: static archive given
// to --dpi-lib; `task $d22_setup()`, `task $d22_hold()` and
// `task $d22_check()` declared as system tasks).
module tb;
    class Base;
        int v;
        Base link;
        function new(int x);
            v = x;
        endfunction
    endclass
    class Derived extends Base;
        function new(int x);
            super.new(x);
        endfunction
    endclass
    Base a;
    Base b;
    function automatic void make();
        a = new(1);
        b = new(2);
        a.link = b;
    endfunction
    function automatic void derived();
        a = Derived::new(3);
    endfunction
    initial begin
        $d22_setup;
        make();
        $d22_hold;
        derived();
        #1 b = null;
        #1 $d22_check;
        $display("a.v=%0d link=%0d", a.v, a.link == null);
        a = null;
        #1 $display("done");
    end
endmodule
