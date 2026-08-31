/*
 Copyright 2019 Alain Dargelas

 Licensed under the Apache License, Version 2.0 (the "License");
 you may not use this file except in compliance with the License.
 You may obtain a copy of the License at

 http://www.apache.org/licenses/LICENSE-2.0

 Unless required by applicable law or agreed to in writing, software
 distributed under the License is distributed on an "AS IS" BASIS,
 WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 See the License for the specific language governing permissions and
 limitations under the License.
 */

/*
 * File:   surelog_c_api.cpp
 *
 * C wrapper around the Surelog C++ API.
 * Provides extern "C" functions that can be called from Rust via FFI.
 */

#include "surelog_c_api.h"

#include <cstdlib>
#include <cstring>
#include <string>

#include "Surelog/API/Surelog.h"
#include "Surelog/CommandLine/CommandLineParser.h"
#include "Surelog/Common/FileSystem.h"
#include "Surelog/Common/PathId.h"
#include "Surelog/Common/SymbolId.h"
#include "Surelog/Design/Design.h"
#include "Surelog/Design/FileContent.h"
#include "Surelog/Design/ModuleInstance.h"
#include "Surelog/Design/VObject.h"
#include "Surelog/ErrorReporting/Error.h"
#include "Surelog/ErrorReporting/ErrorContainer.h"
#include "Surelog/ErrorReporting/ErrorDefinition.h"
#include "Surelog/ErrorReporting/Location.h"
#include "Surelog/SourceCompile/SymbolTable.h"
#include "Surelog/SourceCompile/VObjectTypes.h"

// UHDM — required for VPI operations and the elaboration API
#include <uhdm/ElaboratorListener.h>
#include <uhdm/uhdm.h>
#include <uhdm/uhdm_vpi_user.h>

extern "C" {

/* ─── SymbolTable ─────────────────────────────────────────────────────────── */

SL_SymbolTable sl_create_symbol_table(void) {
  return new SURELOG::SymbolTable();
}

void sl_free_symbol_table(SL_SymbolTable st) {
  delete static_cast<SURELOG::SymbolTable*>(st);
}

/* ─── ErrorContainer ──────────────────────────────────────────────────────── */

SL_ErrorContainer sl_create_error_container(SL_SymbolTable st) {
  return new SURELOG::ErrorContainer(
      static_cast<SURELOG::SymbolTable*>(st));
}

void sl_free_error_container(SL_ErrorContainer ec) {
  delete static_cast<SURELOG::ErrorContainer*>(ec);
}

void sl_errors_print_messages(SL_ErrorContainer ec, int mute) {
  static_cast<SURELOG::ErrorContainer*>(ec)->printMessages(mute != 0);
}

uint32_t sl_errors_get_fatal_count(SL_ErrorContainer ec) {
  return static_cast<uint32_t>(
      static_cast<SURELOG::ErrorContainer*>(ec)->getErrorStats().nbFatal);
}

uint32_t sl_errors_get_syntax_count(SL_ErrorContainer ec) {
  return static_cast<uint32_t>(
      static_cast<SURELOG::ErrorContainer*>(ec)->getErrorStats().nbSyntax);
}

uint32_t sl_errors_get_error_count(SL_ErrorContainer ec) {
  return static_cast<uint32_t>(
      static_cast<SURELOG::ErrorContainer*>(ec)->getErrorStats().nbError);
}

uint32_t sl_errors_get_count(SL_ErrorContainer ec) {
  return static_cast<uint32_t>(
      static_cast<SURELOG::ErrorContainer*>(ec)->getErrors().size());
}

/*
 * Copy diagnostic `index` into *out.  Only entries whose ErrorType has a
 * registered message template are emitted; unknown codes are skipped
 * (return 0) exactly like ErrorContainer::createErrorMessage does.
 *
 * The message is the template text with the FIRST "%s" replaced by the
 * location object name, matching createErrorMessage.  No severity or
 * location prefix is added — the caller decides how to render those.
 */
int sl_errors_get_item(SL_ErrorContainer ec, uint32_t index, SL_Diag* out) {
  SURELOG::ErrorContainer* container =
      static_cast<SURELOG::ErrorContainer*>(ec);
  const auto& errors = container->getErrors();
  if (index >= errors.size()) return 0;

  const SURELOG::Error& err = errors[index];
  const auto& infoMap = SURELOG::ErrorDefinition::getErrorInfoMap();
  auto itr = infoMap.find(err.getType());
  if (itr == infoMap.end()) return 0;
  const SURELOG::ErrorDefinition::ErrorInfo& info = itr->second;

  std::string text = info.m_errorText;
  const auto& locs = err.getLocations();
  uint32_t line = 0;
  uint16_t column = 0;
  std::string file;
  if (!locs.empty()) {
    const SURELOG::Location& loc = locs[0];
    line = loc.m_line;
    column = loc.m_column;
    file = SURELOG::FileSystem::getInstance()->toPath(loc.m_fileId);
    if (loc.m_object != SURELOG::BadSymbolId) {
      const std::string_view objectName =
          container->getSymbolTable()->getSymbol(loc.m_object);
      if (objectName != SURELOG::SymbolTable::getBadSymbol()) {
        const size_t objectOffset = text.find("%s");
        if (objectOffset != std::string::npos) {
          text.replace(objectOffset, 2, objectName);
        }
      }
    }
  }

  out->severity = static_cast<int>(info.m_severity);
  out->file = strdup(file.c_str());
  out->line = line;
  out->col = column;
  out->message = strdup(text.c_str());
  return 1;
}

/* ─── CommandLineParser ───────────────────────────────────────────────────── */

SL_CommandLineParser sl_create_command_line_parser(SL_ErrorContainer ec,
                                                    SL_SymbolTable st) {
  return new SURELOG::CommandLineParser(
      static_cast<SURELOG::ErrorContainer*>(ec),
      static_cast<SURELOG::SymbolTable*>(st),
      /*diffCompMode=*/false,
      /*fileUnit=*/false);
}

void sl_free_command_line_parser(SL_CommandLineParser clp) {
  delete static_cast<SURELOG::CommandLineParser*>(clp);
}

void sl_clp_no_python(SL_CommandLineParser clp) {
  static_cast<SURELOG::CommandLineParser*>(clp)->noPython();
}

int sl_clp_parse_command_line(SL_CommandLineParser clp, int argc,
                               const char** argv) {
  return static_cast<SURELOG::CommandLineParser*>(clp)
             ->parseCommandLine(argc, argv)
         ? 1 : 0;
}

int sl_clp_help(SL_CommandLineParser clp) {
  return static_cast<SURELOG::CommandLineParser*>(clp)->help() ? 1 : 0;
}

int sl_clp_mute_stdout(SL_CommandLineParser clp) {
  return static_cast<SURELOG::CommandLineParser*>(clp)->muteStdout() ? 1 : 0;
}

void sl_clp_set_mute_stdout(SL_CommandLineParser clp) {
  static_cast<SURELOG::CommandLineParser*>(clp)->setMuteStdout();
}

/* ─── Compiler ────────────────────────────────────────────────────────────── */

SL_Compiler sl_start_compiler(SL_CommandLineParser clp) {
  return SURELOG::start_compiler(
      static_cast<SURELOG::CommandLineParser*>(clp));
}

SL_Design sl_get_design(SL_Compiler compiler) {
  return SURELOG::get_design(static_cast<SURELOG::scompiler*>(compiler));
}

void sl_shutdown_compiler(SL_Compiler compiler) {
  SURELOG::shutdown_compiler(static_cast<SURELOG::scompiler*>(compiler));
}

/* ─── Design ──────────────────────────────────────────────────────────────── */

uint32_t sl_design_get_top_instance_count(SL_Design design) {
  return static_cast<uint32_t>(
      static_cast<SURELOG::Design*>(design)
          ->getTopLevelModuleInstances()
          .size());
}

SL_ModuleInstance sl_design_get_top_instance(SL_Design design, uint32_t i) {
  const auto& tops =
      static_cast<SURELOG::Design*>(design)->getTopLevelModuleInstances();
  if (i < tops.size()) return tops[i];
  return nullptr;
}

/* ─── ModuleInstance ──────────────────────────────────────────────────────── */

char* sl_instance_get_full_path_name(SL_ModuleInstance inst) {
  std::string name =
      static_cast<SURELOG::ModuleInstance*>(inst)->getFullPathName();
  return strdup(name.c_str());
}

char* sl_instance_get_file_path(SL_ModuleInstance inst) {
  SURELOG::FileSystem* const fs = SURELOG::FileSystem::getInstance();
  SURELOG::ModuleInstance* mi = static_cast<SURELOG::ModuleInstance*>(inst);
  std::string_view path = fs->toPath(mi->getFileId());
  return strdup(std::string(path).c_str());
}

uint32_t sl_instance_get_child_count(SL_ModuleInstance inst) {
  return static_cast<SURELOG::ModuleInstance*>(inst)->getNbChildren();
}

SL_ModuleInstance sl_instance_get_child(SL_ModuleInstance inst, uint32_t i) {
  return static_cast<SURELOG::ModuleInstance*>(inst)->getChildren(i);
}

/* ─── FileContent ─────────────────────────────────────────────────────────── */

uint32_t sl_design_get_file_content_count(SL_Design design) {
  return static_cast<uint32_t>(
      static_cast<SURELOG::Design*>(design)->getAllFileContents().size());
}

SL_FileContent sl_design_get_file_content(SL_Design design, uint32_t i) {
  auto& files =
      static_cast<SURELOG::Design*>(design)->getAllFileContents();
  if (i >= files.size()) return nullptr;
  return files[i].second;
}

char* sl_file_content_get_path(SL_FileContent fc) {
  auto* f = static_cast<SURELOG::FileContent*>(fc);
  std::string_view path =
      SURELOG::FileSystem::getInstance()->toPath(f->getFileId());
  return strdup(std::string(path).c_str());
}

uint32_t sl_file_content_get_file_id(SL_FileContent fc) {
  auto* f = static_cast<SURELOG::FileContent*>(fc);
  return static_cast<SURELOG::RawPathId>(f->getFileId());
}

uint32_t sl_file_content_get_node_count(SL_FileContent fc) {
  return static_cast<uint32_t>(
      static_cast<SURELOG::FileContent*>(fc)->getVObjects().size());
}

int sl_file_content_get_node(SL_FileContent fc, uint32_t index,
                              SL_VObjectInfo* out) {
  auto* f = static_cast<SURELOG::FileContent*>(fc);
  const auto& objs = f->getVObjects();
  if (index >= objs.size()) return 0;

  const SURELOG::VObject& o = objs[index];
  out->line         = o.m_line;
  out->col          = o.m_column;
  out->end_line     = o.m_endLine;
  out->end_col      = o.m_endColumn;
  out->type_id      = static_cast<uint16_t>(o.m_type);
  out->file_id      = static_cast<SURELOG::RawPathId>(o.m_fileId);
  // Parent NodeId: InvalidRawNodeId (= 0) means "no parent".
  out->parent_index = static_cast<SURELOG::RawNodeId>(o.m_parent);
  out->child_index  = static_cast<SURELOG::RawNodeId>(o.m_child);
  out->sibling_index = static_cast<SURELOG::RawNodeId>(o.m_sibling);
  // getTypeName returns a std::string_view over a static string literal.
  out->type_name    = SURELOG::VObject::getTypeName(o.m_type).data();
  // SymName returns a std::string_view into the SymbolTable intern pool;
  // the pointer is stable for the lifetime of the compiler.
  std::string_view sym = f->SymName(SURELOG::NodeId(index));
  out->symbol_name  = sym.empty() ? nullptr : sym.data();
  return 1;
}

/* ─── Memory ──────────────────────────────────────────────────────────────── */

void sl_free_string(char* s) { free(s); }

/* ─── CommandLineParser extras ───────────────────────────────────────────── */

void sl_clp_set_parse(SL_CommandLineParser clp) {
  static_cast<SURELOG::CommandLineParser*>(clp)->setParse(true);
}

void sl_clp_set_write_pp_output(SL_CommandLineParser clp) {
  static_cast<SURELOG::CommandLineParser*>(clp)->setwritePpOutput(true);
}

void sl_clp_set_compile(SL_CommandLineParser clp) {
  static_cast<SURELOG::CommandLineParser*>(clp)->setCompile(true);
}

void sl_clp_set_elaborate(SL_CommandLineParser clp) {
  static_cast<SURELOG::CommandLineParser*>(clp)->setElaborate(true);
}

void sl_clp_set_elab_uhdm(SL_CommandLineParser clp) {
  static_cast<SURELOG::CommandLineParser*>(clp)->setElabUhdm(true);
}

/* ─── UHDM design ─────────────────────────────────────────────────────────── */

void* sl_get_uhdm_design(SL_Compiler compiler) {
  return SURELOG::get_uhdm_design(static_cast<SURELOG::scompiler*>(compiler));
}

void sl_uhdm_elaborate(void* vpi_design) {
  vpiHandle h = static_cast<vpiHandle>(vpi_design);
  if (!vpi_get(vpiElaborated, h)) {
    UHDM::Serializer serializer;
    UHDM::ElaboratorContext* ctx =
        new UHDM::ElaboratorContext(&serializer, /*muteErrors=*/true);
    ctx->m_elaborator.listenDesigns({h});
    delete ctx;
  }
}

}  // extern "C"
