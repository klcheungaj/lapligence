// Decision A19-D41: definition and configuration properties of instances.
// vpiCellInstance is TRUE for a module compiled between `celldefine and
// `endcelldefine; vpiDefNetType is the `default_nettype in effect for the
// definition (vpiWire by default, vpiTri for tri, vpiNone for none);
// vpiUnconnDrive is vpiPull1 or vpiPull0 under `unconnected_drive and
// vpiHighZ otherwise; vpiAutomatic is the definition's default lifetime;
// vpiDefFile and vpiDefLineNo locate the definition; vpiLibrary is the
// library the definition was compiled into (work for an unmapped source),
// vpiCell the bound cell's name; vpiConfig is NULL with a vpiError when no
// configuration controls the binding; vpiDefDelayMode is vpiDelayModeNone
// (no delay-mode directive); vpiTop is TRUE for the top-level instance.
//
// IEEE 1800-2009 37.10 (diagram, SystemVerilog-1800-2009.txt L57904-
// L57923) lists these properties of an instance; Annex K (L75170, L75186-
// L75190, L75194, L75227): "vpiCellInstance 8 /* cell (Boolean) */",
// "vpiDefNetType 13 /* default net type */", "vpiUnconnDrive 14 /*
// unconnected port drive strength */", "vpiHighZ 1 /* No default drive
// given */", "vpiPull1 2 /* default pull1 drive */", "vpiPull0 3 /* default
// pull0 drive */", "vpiDelayModeNone 1 /* no delay mode specified */",
// "vpiNone 12 /* no default net type (1364-2001) */"; 22.8 (L39749-L39750,
// L39771-L39772), 22.9 (L39778-L39783) and 22.10 (L39793-L39794) give the
// directives; 33.7 (L54034-L54039): "vpiLibrary—the library name into which
// the module was compiled ... vpiCell—the name of the cell bound to the
// module instance ... vpiConfig—the library.cell name of the config
// controlling the binding of the module instance"; 33.3.1 (L53381-L53382):
// "Any file encountered by the compiler that does not match any library's
// file_path_spec shall by default be compiled into a library named work.";
// 37.3.7 (L57542-L57543): "vpiAutomatic is also a property of an instance
// of a module, program, interface, or package, indicating the default
// lifetime for variables of any of its declared tasks/functions."
// The text gives no vpiConfig for a binding no configuration controls; llg
// follows common simulator practice there. Some simulators report vpiWire
// for `default_nettype tri and none, refuse vpiUnconnDrive, report no
// library and an undefined delay mode; llg follows the clause text.
//
// Build A19-D41_instance_definition_properties.c as a VPI application (llg:
// static archive given to --dpi-lib; `task $d41()` declared as a system
// task).
`celldefine
module cellm (input wire a, output wire y);
  assign y = a;
endmodule
`endcelldefine
`default_nettype none
`unconnected_drive pull1
module ndm (input wire a, input wire b, output wire y);
  assign y = a | (b && 1'b0);
endmodule
`nounconnected_drive
`unconnected_drive pull0
`default_nettype tri
module trim (input wire a, output wire y);
  assign y = a;
endmodule
`nounconnected_drive
`default_nettype wire
module automatic autom (input logic a, output logic y);
  assign y = a;
endmodule
module tb;
  logic l = 1'b1;
  wire y1, y2, y3, y4;
  cellm c (.a(l), .y(y1));
  ndm n (.a(l), .y(y2));
  trim t (.a(l), .y(y3));
  autom am (.a(l), .y(y4));
  initial begin
    #1 $d41;
    $display("%b %b %b %b", y1, y2, y3, y4);
    $finish;
  end
endmodule
