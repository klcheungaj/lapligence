// Decision A19-D45: event class properties as VPI objects. See
// docs/lrm_decisions.md for the decision and the expected output.
//
// IEEE 1800-2009 37.29 (SystemVerilog-1800-2009.txt L59582-L59603): a class
// obj iterates vpiVariables and, separately, vpiNamedEvent; 37.27
// (L59446-L59448) gives a class defn the same vpiNamedEvent iteration.
// 37.25 (L59285-L59299): a named event has vpiArray, vpiArrayMember,
// vpiName, vpiFullName, vpiAutomatic, vpiAllocScheme and the value relation
// vpi_put_value(); detail 1 (L59303-L59304): "The vpiWaitingProcesses
// iterator returns all waiting processes, static or dynamic, identified by
// their thread, for that named event." 38.34 (L64239): "Calling
// vpi_put_value() on an object of type vpiNamedEvent shall cause the named
// event to toggle."
//
// Text decides (another simulator differs): an event property is listed by
// vpiNamedEvent, not by vpiVariables; a class definition lists its event
// properties; vpi_put_value triggers the object's event and
// vpiWaitingProcesses lists its waiters. Text silent, llg choice: as for
// other properties, an event property has no vpiFullName (37.17 detail 25)
// and no parent; a static event property is not listed (static class
// properties are not objects).
`timescale 1ns/1ns
class C;
  event e;
  int n;
endclass
class D extends C;
  event f;
endclass
module tb;
  D d;
  time woke = 0;
  initial begin
    d = new;
    fork
      begin @(d.e); woke = $time; end
    join_none
    #1 $d45;
    $display("f triggered=%0d", d.f.triggered);
    #1 $display("f triggered=%0d e woke at %0t", d.f.triggered, woke);
    $finish;
  end
endmodule
