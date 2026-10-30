// Decision A19-D40: lifetime and memory allocation properties of static
// objects. Every module-level variable, element, bit and member of one, and
// every named event, has vpiAutomatic FALSE and vpiAllocScheme
// vpiOtherScheme (3); nets have neither property.
//
// IEEE 1800-2009 37.3.7 (SystemVerilog-1800-2009.txt L57537-L57539):
// "vpiAutomatic is a Boolean property which, when false, means the object
// is static."; L57548-L57550 and L57562-L57564: "The property
// vpiAllocScheme indicates how an object's memory was allocated ... It is an
// enumeration of three possible values: vpiAutomaticScheme,
// vpiDynamicScheme, and vpiOtherScheme. ... vpiAutomaticScheme indicates
// the object is allocated as part of a frame or thread and has the lifetime
// of that frame or thread. vpiDynamicScheme indicates the object was
// allocated in dynamic memory and may be a class object or part thereof.
// For all other objects, vpiAllocScheme shall return vpiOtherScheme."
// 37.17 (diagram, L58712, L58715) and 37.25 (diagram, L59297, L59299) list
// both properties for variables and named events; the net diagram (37.16)
// lists neither, so a net refuses both (decision A19-D26). Some simulators
// refuse vpiAllocScheme of static variables and answer vpiAutomatic of a
// net; llg follows the clause text.
//
// Build A19-D40_static_lifetime_and_allocation.c as a VPI application (llg:
// static archive given to --dpi-lib; `task $d40()` declared as a system
// task).
module tb;
  typedef struct packed { logic a; logic [2:0] b; } pk_t;
  logic [3:0] v = 4'h3;
  bit b = 1'b0;
  int i = 3;
  integer ig = 4;
  shortint si = 5;
  longint li = 6;
  byte by = 7;
  time t = 8;
  real r = 1.5;
  shortreal sr = 2.5;
  logic [1:0][1:0] pa = 4'h1;
  pk_t ps = 4'h5;
  logic [3:0] mem [0:1];
  event ev;
  wire w;
  assign w = v[0];
  initial begin
    mem[0] = 4'h1;
    #1 $d40;
    $display("%0d %0d %0d %0d %0d %0d %0d %0d %0d %0d %0d %0d %0d %0d %0d", v, b, i, ig, si, li,
             by, t, $rtoi(r), $rtoi(sr), pa, ps, mem[0], w, $time);
    -> ev;
    $finish;
  end
endmodule
