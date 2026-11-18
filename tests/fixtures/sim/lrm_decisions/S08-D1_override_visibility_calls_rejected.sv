// Decision S08-D1 (negative): an override that changes visibility still
// warns only, but every call is checked against the visibility of the method
// that the class of the handle declares or inherits. Each call below is
// outside the class hierarchy and names a method that is protected or local
// in the handle's class, so each is an error: a public method made protected
// (d1.f), a protected method made public called through the base handle
// (b2.g), a public method made local (d4.f), a protected pure virtual method
// called through the abstract class (a6.f) and the protected middle of a
// three-level chain (m8.f).
//
// IEEE 1800-2009 8.17 (SystemVerilog-1800-2009.txt L10267-10268): "A member
// identified as local is available only to methods inside the class. Further,
// these local members are not visible within subclasses." L10281-10282: "A
// protected class property or method has all of the characteristics of a
// local member, except that it can be inherited; it is visible to
// subclasses." 8.19 (L10357-10360) gives no rule for an override with other
// encapsulation criteria (see S08-D1_override_visibility).
class B1;
  virtual function int f(); return 1; endfunction
endclass
class D1 extends B1;
  protected virtual function int f(); return 2; endfunction
endclass
class B2;
  protected virtual function int g(); return 10; endfunction
endclass
class D2 extends B2;
  virtual function int g(); return 20; endfunction
endclass
class B4;
  virtual function int f(); return 1; endfunction
endclass
class D4 extends B4;
  local virtual function int f(); return 4; endfunction
endclass
virtual class A6;
  pure virtual protected function int f();
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

module tb;
  D1 d1; D2 d2; B2 b2; D4 d4; C6 c6; A6 a6; D8 d8; M8 m8;
  initial begin
    d1 = new; d2 = new; b2 = d2; d4 = new; c6 = new; a6 = c6; d8 = new; m8 = d8;
    $display("%0d", d1.f());
    $display("%0d", b2.g());
    $display("%0d", d4.f());
    $display("%0d", a6.f());
    $display("%0d", m8.f());
    $finish;
  end
endmodule
