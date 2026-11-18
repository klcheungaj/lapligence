// Decision S08-D1: an override of a virtual method may change its visibility
// (local, protected or public). The frontend warns at the override; the
// override still overrides, so calls dispatch to it through any handle, and
// each call is checked against the visibility of the method that the class of
// the handle declares (or inherits). Covers protected -> public, public ->
// protected, a local method in the base or the derived class, extern
// methods, a pure virtual method of an abstract class, a three-level chain
// and super calls.
//
// IEEE 1800-2009 8.19 (SystemVerilog-1800-2009.txt L10357-10360): "Virtual
// methods provide prototypes for the methods that later override them, i.e.,
// all of the information generally found on the first line of a method
// declaration: the encapsulation criteria, the type and number of arguments,
// and the return type if it is needed. Later, when subclasses override virtual
// methods, they shall follow the prototype exactly by having matching return
// types and matching argument names, types, and directions." The required
// matches name return and argument facts only; the text gives no rule for an
// override with other encapsulation criteria. 8.17 (L10267-10268): "A member
// identified as local is available only to methods inside the class. Further,
// these local members are not visible within subclasses."
class B1;
  virtual function int f(); return 1; endfunction
  function int callf(); return f(); endfunction
endclass
class D1 extends B1;
  protected virtual function int f(); return 2; endfunction
endclass

class B2;
  protected virtual function int g(); return 10; endfunction
  function int callg(); return g(); endfunction
endclass
class D2 extends B2;
  virtual function int g(); return 20; endfunction
endclass

class B3;
  local virtual function int f(); return 1; endfunction
  function int callf(); return f(); endfunction
endclass
class D3 extends B3;
  virtual function int f(); return 3; endfunction
endclass

class B4;
  virtual function int f(); return 1; endfunction
  function int callf(); return f(); endfunction
endclass
class D4 extends B4;
  local virtual function int f(); return 4; endfunction
  function int dcall(); return f(); endfunction
endclass

class B5;
  extern virtual function int f();
  extern protected virtual task t(output int x);
  task callt(output int x); t(x); endtask
endclass
function int B5::f(); return 1; endfunction
task B5::t(output int x); x = 5; endtask
class D5 extends B5;
  extern protected virtual function int f();
  extern virtual task t(output int x);
endclass
function int D5::f(); return 50; endfunction
task D5::t(output int x); x = 51; endtask

virtual class A6;
  pure virtual protected function int f();
  function int callf(); return f(); endfunction
endclass
class C6 extends A6;
  virtual function int f(); return 6; endfunction
endclass

class B8;
  virtual function int f(); return 1; endfunction
endclass
class M8 extends B8;
  protected virtual function int f(); return 2; endfunction
endclass
class D8 extends M8;
  virtual function int f(); return 8; endfunction
endclass

class B9;
  protected virtual function int f(); return 1; endfunction
endclass
class D9 extends B9;
  virtual function int f(); return 10 + super.f(); endfunction
endclass
class E9 extends D9;
  protected virtual function int f(); return 100 + super.f(); endfunction
  function int ecall(); return f(); endfunction
endclass

module tb;
  D1 d1; B1 b1; D2 d2; B2 b2; D3 d3; B3 b3; D4 d4; B4 b4; D5 d5; B5 b5;
  C6 c6; A6 a6; D8 d8; B8 b8; E9 e9; D9 d9;
  int x, y;
  initial begin
    d1 = new; b1 = d1; d2 = new; b2 = d2; d3 = new; b3 = d3;
    d4 = new; b4 = d4; d5 = new; b5 = d5; c6 = new; a6 = c6;
    d8 = new; b8 = d8; e9 = new; d9 = e9;
    $display("public->protected: b1.f=%0d b1.callf=%0d", b1.f(), b1.callf());
    $display("protected->public: d2.g=%0d b2.callg=%0d", d2.g(), b2.callg());
    $display("local base: d3.f=%0d b3.callf=%0d", d3.f(), b3.callf());
    $display("local derived: b4.f=%0d b4.callf=%0d d4.dcall=%0d", b4.f(), b4.callf(), d4.dcall());
    b5.callt(x);
    d5.t(y);
    $display("extern: b5.f=%0d x=%0d y=%0d", b5.f(), x, y);
    $display("pure: c6.f=%0d a6.callf=%0d", c6.f(), a6.callf());
    $display("three levels: d8.f=%0d b8.f=%0d", d8.f(), b8.f());
    $display("super: d9.f=%0d e9.ecall=%0d", d9.f(), e9.ecall());
    $finish;
  end
endmodule
