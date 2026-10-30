// Decision A19-D27: unnamed blocks, for and foreach statements as scopes.
// An unnamed begin or fork that declares an item, a for statement that
// declares its loop variables and every foreach statement are scopes: they
// keep their statement types (vpiBegin, vpiFork, vpiFor, vpiForeachStmt),
// hold their declarations (vpiVariables, vpiInternalScope) and are the
// vpiScope of their declarations and statements. Each is named
// unnamed$$_<n>, n counting from 0, in source order, the unnamed scopes
// whose nearest named scope (instance, generate scope, task, function or
// named block) is the same; vpiFullName extends its scope's name, and
// vpi_handle_by_name finds it and what it declares. A begin or fork without
// a declaration is not a scope: vpiFullName fails, and the scope iterations
// return NULL without an error. A for statement without declarations
// refuses them with an error. Named blocks of tasks and functions are
// scopes like those of processes.
//
// IEEE 1800-2009 37.12 (SystemVerilog-1800-2009.txt L58069-L58070): "An
// unnamed begin or unnamed fork shall be a scope if and only if it directly
// contains a block item declaration such as a variable declaration or type
// declaration."; L58081-L58082: "A for statement shall be a scope if and
// only if the vpiLocalVarDecls property returns TRUE. In this case, the
// scope of each loop control variable shall be the for statement."; L58083:
// "The scope of each loop control variable in a foreach stmt shall be the
// foreach stmt." 9.3.4 (L11578-L11580): "An unnamed block creates a new
// hierarchy scope only if it directly contains a block item declaration
// ... This hierarchy scope is unnamed". The text does not name an unnamed
// scope; llg follows industry-standard behaviour for the generated names.
// Some simulators report such a block, and a block wrapped around a for or
// foreach statement, as a vpiNamedBegin; llg keeps the statement types and
// makes the for and foreach statements the scopes, as the clause says.
//
// Build A19-D27_statement_scopes.c as a VPI application (llg: static
// archive given to --dpi-lib; `task $d27()` declared as a system task).
module tb;
  int arr [2];
  int x;
  task automatic t;
    fork : par
      x = 1;
    join
  endtask
  initial begin
    static int a = 1;
    begin
      x = a;
    end
    begin
      static int c = 3;
      x = c;
    end
    for (int i = 0; i < 2; i++) x += i;
    for (x = 0; x < 1; x++) arr[0] = x;
    foreach (arr[j]) x += j;
    fork
      static int b = 2;
      x = b;
    join
    t();
    #1 $d27;
    $finish;
  end
endmodule
