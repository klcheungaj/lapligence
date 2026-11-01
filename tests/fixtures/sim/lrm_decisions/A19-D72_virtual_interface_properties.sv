// Decision A19-D72: a virtual interface property of a class is listed by
// vpiInterfaceDecl of the class definition and of each class object, as a
// reference object (vpiRefObj) named after the property, with vpiVirtual
// TRUE. vpiActual of an object's property is the interface instance the
// property currently names, NULL while it names none; a definition's
// property names no instance, so its vpiActual is an error. The property
// has no value.
//
// IEEE 1800-2009 37.27 detail 7 and 37.29 detail 7
// (SystemVerilog-1800-2009.txt L59481, L59651): "The vpiInterfaceDecl
// iteration returns the virtual interfaces of the class object." 37.15
// draws the reference object with "-> virtual bool: vpiVirtual" and a
// vpiActual relation to the interface it refers to. Industry-standard
// behaviour.
//
// Build A19-D72_virtual_interface_properties.c as a VPI application (llg:
// static archive given to --dpi-lib; `task $d72()` declared as a system
// task).
interface ifc;
    logic s;
endinterface

class K;
    virtual ifc vi;
    int y;
endclass

module tb;
    ifc i0 ();
    ifc i1 ();
    K k;
    initial begin
        k = new;
        $d72;
        k.vi = i1;
        $d72;
        k.vi = i0;
        $d72;
        $finish;
    end
endmodule
