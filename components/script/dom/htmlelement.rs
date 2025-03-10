/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::HashSet;
use std::default::Default;
use std::rc::Rc;

use dom_struct::dom_struct;
use html5ever::{local_name, namespace_url, ns, LocalName, Prefix};
use js::rust::HandleObject;
use script_layout_interface::message::QueryMsg;
use style::attr::AttrValue;
use style_traits::dom::ElementState;

use crate::dom::activation::Activatable;
use crate::dom::attr::Attr;
use crate::dom::bindings::codegen::Bindings::EventHandlerBinding::{
    EventHandlerNonNull, OnErrorEventHandlerNonNull,
};
use crate::dom::bindings::codegen::Bindings::HTMLElementBinding::HTMLElementMethods;
use crate::dom::bindings::codegen::Bindings::HTMLLabelElementBinding::HTMLLabelElementMethods;
use crate::dom::bindings::codegen::Bindings::NodeBinding::Node_Binding::NodeMethods;
use crate::dom::bindings::codegen::Bindings::WindowBinding::WindowMethods;
use crate::dom::bindings::error::{Error, ErrorResult};
use crate::dom::bindings::inheritance::{Castable, ElementTypeId, HTMLElementTypeId, NodeTypeId};
use crate::dom::bindings::root::{Dom, DomRoot, MutNullableDom};
use crate::dom::bindings::str::DOMString;
use crate::dom::cssstyledeclaration::{CSSModificationAccess, CSSStyleDeclaration, CSSStyleOwner};
use crate::dom::document::{Document, FocusType};
use crate::dom::documentfragment::DocumentFragment;
use crate::dom::domstringmap::DOMStringMap;
use crate::dom::element::{AttributeMutation, Element};
use crate::dom::event::Event;
use crate::dom::eventtarget::EventTarget;
use crate::dom::htmlbodyelement::HTMLBodyElement;
use crate::dom::htmlbrelement::HTMLBRElement;
use crate::dom::htmldetailselement::HTMLDetailsElement;
use crate::dom::htmlframesetelement::HTMLFrameSetElement;
use crate::dom::htmlhtmlelement::HTMLHtmlElement;
use crate::dom::htmlinputelement::{HTMLInputElement, InputType};
use crate::dom::htmllabelelement::HTMLLabelElement;
use crate::dom::htmltextareaelement::HTMLTextAreaElement;
use crate::dom::node::{document_from_node, window_from_node, Node, ShadowIncluding};
use crate::dom::text::Text;
use crate::dom::virtualmethods::VirtualMethods;

#[dom_struct]
pub struct HTMLElement {
    element: Element,
    style_decl: MutNullableDom<CSSStyleDeclaration>,
    dataset: MutNullableDom<DOMStringMap>,
}

impl HTMLElement {
    pub fn new_inherited(
        tag_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> HTMLElement {
        HTMLElement::new_inherited_with_state(ElementState::empty(), tag_name, prefix, document)
    }

    pub fn new_inherited_with_state(
        state: ElementState,
        tag_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> HTMLElement {
        HTMLElement {
            element: Element::new_inherited_with_state(
                state,
                tag_name,
                ns!(html),
                prefix,
                document,
            ),
            style_decl: Default::default(),
            dataset: Default::default(),
        }
    }

    #[allow(unrooted_must_root)]
    pub fn new(
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
        proto: Option<HandleObject>,
    ) -> DomRoot<HTMLElement> {
        Node::reflect_node_with_proto(
            Box::new(HTMLElement::new_inherited(local_name, prefix, document)),
            document,
            proto,
        )
    }

    fn is_body_or_frameset(&self) -> bool {
        let eventtarget = self.upcast::<EventTarget>();
        eventtarget.is::<HTMLBodyElement>() || eventtarget.is::<HTMLFrameSetElement>()
    }
}

impl HTMLElementMethods for HTMLElement {
    // https://html.spec.whatwg.org/multipage/#the-style-attribute
    fn Style(&self) -> DomRoot<CSSStyleDeclaration> {
        self.style_decl.or_init(|| {
            let global = window_from_node(self);
            CSSStyleDeclaration::new(
                &global,
                CSSStyleOwner::Element(Dom::from_ref(self.upcast())),
                None,
                CSSModificationAccess::ReadWrite,
            )
        })
    }

    // https://html.spec.whatwg.org/multipage/#attr-title
    make_getter!(Title, "title");
    // https://html.spec.whatwg.org/multipage/#attr-title
    make_setter!(SetTitle, "title");

    // https://html.spec.whatwg.org/multipage/#attr-lang
    make_getter!(Lang, "lang");
    // https://html.spec.whatwg.org/multipage/#attr-lang
    make_setter!(SetLang, "lang");

    // https://html.spec.whatwg.org/multipage/#the-dir-attribute
    make_enumerated_getter!(Dir, "dir", "", "ltr" | "rtl" | "auto");
    // https://html.spec.whatwg.org/multipage/#the-dir-attribute
    make_setter!(SetDir, "dir");

    // https://html.spec.whatwg.org/multipage/#dom-hidden
    make_bool_getter!(Hidden, "hidden");
    // https://html.spec.whatwg.org/multipage/#dom-hidden
    make_bool_setter!(SetHidden, "hidden");

    // https://html.spec.whatwg.org/multipage/#globaleventhandlers
    global_event_handlers!(NoOnload);

    // https://html.spec.whatwg.org/multipage/#documentandelementeventhandlers
    document_and_element_event_handlers!();

    // https://html.spec.whatwg.org/multipage/#dom-dataset
    fn Dataset(&self) -> DomRoot<DOMStringMap> {
        self.dataset.or_init(|| DOMStringMap::new(self))
    }

    // https://html.spec.whatwg.org/multipage/#handler-onerror
    fn GetOnerror(&self) -> Option<Rc<OnErrorEventHandlerNonNull>> {
        if self.is_body_or_frameset() {
            let document = document_from_node(self);
            if document.has_browsing_context() {
                document.window().GetOnerror()
            } else {
                None
            }
        } else {
            self.upcast::<EventTarget>()
                .get_event_handler_common("error")
        }
    }

    // https://html.spec.whatwg.org/multipage/#handler-onerror
    fn SetOnerror(&self, listener: Option<Rc<OnErrorEventHandlerNonNull>>) {
        if self.is_body_or_frameset() {
            let document = document_from_node(self);
            if document.has_browsing_context() {
                document.window().SetOnerror(listener)
            }
        } else {
            // special setter for error
            self.upcast::<EventTarget>()
                .set_error_event_handler("error", listener)
        }
    }

    // https://html.spec.whatwg.org/multipage/#handler-onload
    fn GetOnload(&self) -> Option<Rc<EventHandlerNonNull>> {
        if self.is_body_or_frameset() {
            let document = document_from_node(self);
            if document.has_browsing_context() {
                document.window().GetOnload()
            } else {
                None
            }
        } else {
            self.upcast::<EventTarget>()
                .get_event_handler_common("load")
        }
    }

    // https://html.spec.whatwg.org/multipage/#handler-onload
    fn SetOnload(&self, listener: Option<Rc<EventHandlerNonNull>>) {
        if self.is_body_or_frameset() {
            let document = document_from_node(self);
            if document.has_browsing_context() {
                document.window().SetOnload(listener)
            }
        } else {
            self.upcast::<EventTarget>()
                .set_event_handler_common("load", listener)
        }
    }

    // https://html.spec.whatwg.org/multipage/#handler-onblur
    fn GetOnblur(&self) -> Option<Rc<EventHandlerNonNull>> {
        if self.is_body_or_frameset() {
            let document = document_from_node(self);
            if document.has_browsing_context() {
                document.window().GetOnblur()
            } else {
                None
            }
        } else {
            self.upcast::<EventTarget>()
                .get_event_handler_common("blur")
        }
    }

    // https://html.spec.whatwg.org/multipage/#handler-onblur
    fn SetOnblur(&self, listener: Option<Rc<EventHandlerNonNull>>) {
        if self.is_body_or_frameset() {
            let document = document_from_node(self);
            if document.has_browsing_context() {
                document.window().SetOnblur(listener)
            }
        } else {
            self.upcast::<EventTarget>()
                .set_event_handler_common("blur", listener)
        }
    }

    // https://html.spec.whatwg.org/multipage/#handler-onfocus
    fn GetOnfocus(&self) -> Option<Rc<EventHandlerNonNull>> {
        if self.is_body_or_frameset() {
            let document = document_from_node(self);
            if document.has_browsing_context() {
                document.window().GetOnfocus()
            } else {
                None
            }
        } else {
            self.upcast::<EventTarget>()
                .get_event_handler_common("focus")
        }
    }

    // https://html.spec.whatwg.org/multipage/#handler-onfocus
    fn SetOnfocus(&self, listener: Option<Rc<EventHandlerNonNull>>) {
        if self.is_body_or_frameset() {
            let document = document_from_node(self);
            if document.has_browsing_context() {
                document.window().SetOnfocus(listener)
            }
        } else {
            self.upcast::<EventTarget>()
                .set_event_handler_common("focus", listener)
        }
    }

    // https://html.spec.whatwg.org/multipage/#handler-onresize
    fn GetOnresize(&self) -> Option<Rc<EventHandlerNonNull>> {
        if self.is_body_or_frameset() {
            let document = document_from_node(self);
            if document.has_browsing_context() {
                document.window().GetOnresize()
            } else {
                None
            }
        } else {
            self.upcast::<EventTarget>()
                .get_event_handler_common("resize")
        }
    }

    // https://html.spec.whatwg.org/multipage/#handler-onresize
    fn SetOnresize(&self, listener: Option<Rc<EventHandlerNonNull>>) {
        if self.is_body_or_frameset() {
            let document = document_from_node(self);
            if document.has_browsing_context() {
                document.window().SetOnresize(listener)
            }
        } else {
            self.upcast::<EventTarget>()
                .set_event_handler_common("resize", listener)
        }
    }

    // https://html.spec.whatwg.org/multipage/#handler-onscroll
    fn GetOnscroll(&self) -> Option<Rc<EventHandlerNonNull>> {
        if self.is_body_or_frameset() {
            let document = document_from_node(self);
            if document.has_browsing_context() {
                document.window().GetOnscroll()
            } else {
                None
            }
        } else {
            self.upcast::<EventTarget>()
                .get_event_handler_common("scroll")
        }
    }

    // https://html.spec.whatwg.org/multipage/#handler-onscroll
    fn SetOnscroll(&self, listener: Option<Rc<EventHandlerNonNull>>) {
        if self.is_body_or_frameset() {
            let document = document_from_node(self);
            if document.has_browsing_context() {
                document.window().SetOnscroll(listener)
            }
        } else {
            self.upcast::<EventTarget>()
                .set_event_handler_common("scroll", listener)
        }
    }

    // https://html.spec.whatwg.org/multipage/#attr-itemtype
    fn Itemtypes(&self) -> Option<Vec<DOMString>> {
        let atoms = self
            .element
            .get_tokenlist_attribute(&local_name!("itemtype"));

        if atoms.is_empty() {
            return None;
        }

        let mut item_attr_values = HashSet::new();
        for attr_value in &atoms {
            item_attr_values.insert(DOMString::from(String::from(attr_value.trim())));
        }

        Some(item_attr_values.into_iter().collect())
    }

    // https://html.spec.whatwg.org/multipage/#names:-the-itemprop-attribute
    fn PropertyNames(&self) -> Option<Vec<DOMString>> {
        let atoms = self
            .element
            .get_tokenlist_attribute(&local_name!("itemprop"));

        if atoms.is_empty() {
            return None;
        }

        let mut item_attr_values = HashSet::new();
        for attr_value in &atoms {
            item_attr_values.insert(DOMString::from(String::from(attr_value.trim())));
        }

        Some(item_attr_values.into_iter().collect())
    }

    // https://html.spec.whatwg.org/multipage/#dom-click
    fn Click(&self) {
        let element = self.upcast::<Element>();
        if element.disabled_state() {
            return;
        }
        if element.click_in_progress() {
            return;
        }
        element.set_click_in_progress(true);

        self.upcast::<Node>()
            .fire_synthetic_mouse_event_not_trusted(DOMString::from("click"));
        element.set_click_in_progress(false);
    }

    // https://html.spec.whatwg.org/multipage/#dom-focus
    fn Focus(&self) {
        // TODO: Mark the element as locked for focus and run the focusing steps.
        // https://html.spec.whatwg.org/multipage/#focusing-steps
        let document = document_from_node(self);
        document.request_focus(Some(self.upcast()), FocusType::Element);
    }

    // https://html.spec.whatwg.org/multipage/#dom-blur
    fn Blur(&self) {
        // TODO: Run the unfocusing steps.
        if !self.upcast::<Element>().focus_state() {
            return;
        }
        // https://html.spec.whatwg.org/multipage/#unfocusing-steps
        let document = document_from_node(self);
        document.request_focus(None, FocusType::Element);
    }

    // https://drafts.csswg.org/cssom-view/#dom-htmlelement-offsetparent
    fn GetOffsetParent(&self) -> Option<DomRoot<Element>> {
        if self.is::<HTMLBodyElement>() || self.is::<HTMLHtmlElement>() {
            return None;
        }

        let node = self.upcast::<Node>();
        let window = window_from_node(self);
        let (element, _) = window.offset_parent_query(node);

        element
    }

    // https://drafts.csswg.org/cssom-view/#dom-htmlelement-offsettop
    fn OffsetTop(&self) -> i32 {
        if self.is::<HTMLBodyElement>() {
            return 0;
        }

        let node = self.upcast::<Node>();
        let window = window_from_node(self);
        let (_, rect) = window.offset_parent_query(node);

        rect.origin.y.to_nearest_px()
    }

    // https://drafts.csswg.org/cssom-view/#dom-htmlelement-offsetleft
    fn OffsetLeft(&self) -> i32 {
        if self.is::<HTMLBodyElement>() {
            return 0;
        }

        let node = self.upcast::<Node>();
        let window = window_from_node(self);
        let (_, rect) = window.offset_parent_query(node);

        rect.origin.x.to_nearest_px()
    }

    // https://drafts.csswg.org/cssom-view/#dom-htmlelement-offsetwidth
    fn OffsetWidth(&self) -> i32 {
        let node = self.upcast::<Node>();
        let window = window_from_node(self);
        let (_, rect) = window.offset_parent_query(node);

        rect.size.width.to_nearest_px()
    }

    // https://drafts.csswg.org/cssom-view/#dom-htmlelement-offsetheight
    fn OffsetHeight(&self) -> i32 {
        let node = self.upcast::<Node>();
        let window = window_from_node(self);
        let (_, rect) = window.offset_parent_query(node);

        rect.size.height.to_nearest_px()
    }

    // https://html.spec.whatwg.org/multipage/#the-innertext-idl-attribute
    fn InnerText(&self) -> DOMString {
        let node = self.upcast::<Node>();
        let window = window_from_node(node);
        let element = self.upcast::<Element>();

        // Step 1.
        let element_not_rendered = !node.is_connected() || !element.has_css_layout_box();
        if element_not_rendered {
            return node.GetTextContent().unwrap();
        }

        window.layout_reflow(QueryMsg::ElementInnerTextQuery(
            node.to_trusted_node_address(),
        ));
        DOMString::from(window.layout().element_inner_text())
    }

    // https://html.spec.whatwg.org/multipage/#the-innertext-idl-attribute
    fn SetInnerText(&self, input: DOMString) {
        // Step 1.
        let document = document_from_node(self);

        // Step 2.
        let fragment = DocumentFragment::new(&document);

        // Step 3. The given value is already named 'input'.

        // Step 4.
        let mut position = input.chars().peekable();

        // Step 5.
        let mut text = String::new();

        // Step 6.
        while let Some(ch) = position.next() {
            match ch {
                '\u{000A}' | '\u{000D}' => {
                    if ch == '\u{000D}' && position.peek() == Some(&'\u{000A}') {
                        // a \r\n pair should only generate one <br>,
                        // so just skip the \r.
                        position.next();
                    }

                    if !text.is_empty() {
                        append_text_node_to_fragment(&document, &fragment, text);
                        text = String::new();
                    }

                    let br = HTMLBRElement::new(local_name!("br"), None, &document, None);
                    fragment.upcast::<Node>().AppendChild(&br.upcast()).unwrap();
                },
                _ => {
                    text.push(ch);
                },
            }
        }

        if !text.is_empty() {
            append_text_node_to_fragment(&document, &fragment, text);
        }

        // Step 7.
        Node::replace_all(Some(fragment.upcast()), self.upcast::<Node>());
    }

    // https://html.spec.whatwg.org/multipage/#dom-translate
    fn Translate(&self) -> bool {
        self.upcast::<Element>().is_translate_enabled()
    }

    // https://html.spec.whatwg.org/multipage/#dom-translate
    fn SetTranslate(&self, yesno: bool) {
        self.upcast::<Element>().set_string_attribute(
            &html5ever::local_name!("translate"),
            match yesno {
                true => DOMString::from("yes"),
                false => DOMString::from("no"),
            },
        );
    }

    // https://html.spec.whatwg.org/multipage/#dom-contenteditable
    fn ContentEditable(&self) -> DOMString {
        // TODO: https://github.com/servo/servo/issues/12776
        self.upcast::<Element>()
            .get_attribute(&ns!(), &local_name!("contenteditable"))
            .map(|attr| DOMString::from(&**attr.value()))
            .unwrap_or_else(|| DOMString::from("inherit"))
    }

    // https://html.spec.whatwg.org/multipage/#dom-contenteditable
    fn SetContentEditable(&self, _: DOMString) {
        // TODO: https://github.com/servo/servo/issues/12776
        warn!("The contentEditable attribute is not implemented yet");
    }

    // https://html.spec.whatwg.org/multipage/#dom-contenteditable
    fn IsContentEditable(&self) -> bool {
        // TODO: https://github.com/servo/servo/issues/12776
        false
    }
}

fn append_text_node_to_fragment(document: &Document, fragment: &DocumentFragment, text: String) {
    let text = Text::new(DOMString::from(text), document);
    fragment
        .upcast::<Node>()
        .AppendChild(&text.upcast())
        .unwrap();
}

// https://html.spec.whatwg.org/multipage/#attr-data-*

static DATA_PREFIX: &str = "data-";
static DATA_HYPHEN_SEPARATOR: char = '\x2d';

fn is_ascii_uppercase(c: char) -> bool {
    'A' <= c && c <= 'Z'
}

fn is_ascii_lowercase(c: char) -> bool {
    'a' <= c && c <= 'w'
}

fn to_snake_case(name: DOMString) -> DOMString {
    let mut attr_name = String::with_capacity(name.len() + DATA_PREFIX.len());
    attr_name.push_str(DATA_PREFIX);
    for ch in name.chars() {
        if is_ascii_uppercase(ch) {
            attr_name.push(DATA_HYPHEN_SEPARATOR);
            attr_name.push(ch.to_ascii_lowercase());
        } else {
            attr_name.push(ch);
        }
    }
    DOMString::from(attr_name)
}

// https://html.spec.whatwg.org/multipage/#attr-data-*
// if this attribute is in snake case with a data- prefix,
// this function returns a name converted to camel case
// without the data prefix.

fn to_camel_case(name: &str) -> Option<DOMString> {
    if !name.starts_with(DATA_PREFIX) {
        return None;
    }
    let name = &name[5..];
    let has_uppercase = name.chars().any(|curr_char| is_ascii_uppercase(curr_char));
    if has_uppercase {
        return None;
    }
    let mut result = String::with_capacity(name.len().saturating_sub(DATA_PREFIX.len()));
    let mut name_chars = name.chars();
    while let Some(curr_char) = name_chars.next() {
        //check for hyphen followed by character
        if curr_char == DATA_HYPHEN_SEPARATOR {
            if let Some(next_char) = name_chars.next() {
                if is_ascii_lowercase(next_char) {
                    result.push(next_char.to_ascii_uppercase());
                } else {
                    result.push(curr_char);
                    result.push(next_char);
                }
            } else {
                result.push(curr_char);
            }
        } else {
            result.push(curr_char);
        }
    }
    Some(DOMString::from(result))
}

impl HTMLElement {
    pub fn set_custom_attr(&self, name: DOMString, value: DOMString) -> ErrorResult {
        if name
            .chars()
            .skip_while(|&ch| ch != '\u{2d}')
            .nth(1)
            .map_or(false, |ch| ch >= 'a' && ch <= 'z')
        {
            return Err(Error::Syntax);
        }
        self.upcast::<Element>()
            .set_custom_attribute(to_snake_case(name), value)
    }

    pub fn get_custom_attr(&self, local_name: DOMString) -> Option<DOMString> {
        // FIXME(ajeffrey): Convert directly from DOMString to LocalName
        let local_name = LocalName::from(to_snake_case(local_name));
        self.upcast::<Element>()
            .get_attribute(&ns!(), &local_name)
            .map(|attr| {
                DOMString::from(&**attr.value()) // FIXME(ajeffrey): Convert directly from AttrValue to DOMString
            })
    }

    pub fn delete_custom_attr(&self, local_name: DOMString) {
        // FIXME(ajeffrey): Convert directly from DOMString to LocalName
        let local_name = LocalName::from(to_snake_case(local_name));
        self.upcast::<Element>()
            .remove_attribute(&ns!(), &local_name);
    }

    // https://html.spec.whatwg.org/multipage/#category-label
    pub fn is_labelable_element(&self) -> bool {
        match self.upcast::<Node>().type_id() {
            NodeTypeId::Element(ElementTypeId::HTMLElement(type_id)) => match type_id {
                HTMLElementTypeId::HTMLInputElement => {
                    self.downcast::<HTMLInputElement>().unwrap().input_type() != InputType::Hidden
                },
                HTMLElementTypeId::HTMLButtonElement |
                HTMLElementTypeId::HTMLMeterElement |
                HTMLElementTypeId::HTMLOutputElement |
                HTMLElementTypeId::HTMLProgressElement |
                HTMLElementTypeId::HTMLSelectElement |
                HTMLElementTypeId::HTMLTextAreaElement => true,
                _ => false,
            },
            _ => false,
        }
    }

    // https://html.spec.whatwg.org/multipage/#category-listed
    pub fn is_listed_element(&self) -> bool {
        match self.upcast::<Node>().type_id() {
            NodeTypeId::Element(ElementTypeId::HTMLElement(type_id)) => match type_id {
                HTMLElementTypeId::HTMLButtonElement |
                HTMLElementTypeId::HTMLFieldSetElement |
                HTMLElementTypeId::HTMLInputElement |
                HTMLElementTypeId::HTMLObjectElement |
                HTMLElementTypeId::HTMLOutputElement |
                HTMLElementTypeId::HTMLSelectElement |
                HTMLElementTypeId::HTMLTextAreaElement => true,
                _ => false,
            },
            _ => false,
        }
    }

    pub fn supported_prop_names_custom_attr(&self) -> Vec<DOMString> {
        let element = self.upcast::<Element>();
        element
            .attrs()
            .iter()
            .filter_map(|attr| {
                let raw_name = attr.local_name();
                to_camel_case(&raw_name)
            })
            .collect()
    }

    // https://html.spec.whatwg.org/multipage/#dom-lfe-labels
    // This gets the nth label in tree order.
    pub fn label_at(&self, index: u32) -> Option<DomRoot<Node>> {
        let element = self.upcast::<Element>();

        // Traverse entire tree for <label> elements that have
        // this as their control.
        // There is room for performance optimization, as we don't need
        // the actual result of GetControl, only whether the result
        // would match self.
        // (Even more room for performance optimization: do what
        // nodelist ChildrenList does and keep a mutation-aware cursor
        // around; this may be hard since labels need to keep working
        // even as they get detached into a subtree and reattached to
        // a document.)
        let root_element = element.root_element();
        let root_node = root_element.upcast::<Node>();
        root_node
            .traverse_preorder(ShadowIncluding::No)
            .filter_map(DomRoot::downcast::<HTMLLabelElement>)
            .filter(|elem| match elem.GetControl() {
                Some(control) => &*control == self,
                _ => false,
            })
            .nth(index as usize)
            .map(|n| DomRoot::from_ref(n.upcast::<Node>()))
    }

    // https://html.spec.whatwg.org/multipage/#dom-lfe-labels
    // This counts the labels of the element, to support NodeList::Length
    pub fn labels_count(&self) -> u32 {
        // see label_at comments about performance
        let element = self.upcast::<Element>();
        let root_element = element.root_element();
        let root_node = root_element.upcast::<Node>();
        root_node
            .traverse_preorder(ShadowIncluding::No)
            .filter_map(DomRoot::downcast::<HTMLLabelElement>)
            .filter(|elem| match elem.GetControl() {
                Some(control) => &*control == self,
                _ => false,
            })
            .count() as u32
    }

    // https://html.spec.whatwg.org/multipage/#the-directionality.
    // returns Some if can infer direction by itself or from child nodes
    // returns None if requires to go up to parent
    pub fn directionality(&self) -> Option<String> {
        let element_direction: &str = &self.Dir();

        if element_direction == "ltr" {
            return Some("ltr".to_owned());
        }

        if element_direction == "rtl" {
            return Some("rtl".to_owned());
        }

        if let Some(input) = self.downcast::<HTMLInputElement>() {
            if input.input_type() == InputType::Tel {
                return Some("ltr".to_owned());
            }
        }

        if element_direction == "auto" {
            if let Some(directionality) = self
                .downcast::<HTMLInputElement>()
                .and_then(|input| input.auto_directionality())
            {
                return Some(directionality);
            }

            if let Some(area) = self.downcast::<HTMLTextAreaElement>() {
                return Some(area.auto_directionality());
            }
        }

        // TODO(NeverHappened): Implement condition
        // If the element's dir attribute is in the auto state OR
        // If the element is a bdi element and the dir attribute is not in a defined state
        // (i.e. it is not present or has an invalid value)
        // Requires bdi element implementation (https://html.spec.whatwg.org/multipage/#the-bdi-element)

        None
    }

    // https://html.spec.whatwg.org/multipage/#the-summary-element:activation-behaviour
    pub fn summary_activation_behavior(&self) {
        // Step 1
        if !self.is_summary_for_its_parent_details() {
            return;
        }

        // Step 2
        let parent_details = self.upcast::<Node>().GetParentNode().unwrap();

        // Step 3
        parent_details
            .downcast::<HTMLDetailsElement>()
            .unwrap()
            .toggle();
    }

    // https://html.spec.whatwg.org/multipage/#summary-for-its-parent-details
    fn is_summary_for_its_parent_details(&self) -> bool {
        // Step 1
        let summary_node = self.upcast::<Node>();
        if !summary_node.has_parent() {
            return false;
        }

        // Step 2
        let parent = &summary_node.GetParentNode().unwrap();

        // Step 3
        if !parent.is::<HTMLDetailsElement>() {
            return false;
        }

        // Step 4 & 5
        let first_summary_element = parent
            .child_elements()
            .find(|el| el.local_name() == &local_name!("summary"));
        match first_summary_element {
            Some(first_summary) => &*first_summary == self.upcast::<Element>(),
            None => false,
        }
    }
}

impl VirtualMethods for HTMLElement {
    fn super_type(&self) -> Option<&dyn VirtualMethods> {
        Some(self.upcast::<Element>() as &dyn VirtualMethods)
    }

    fn attribute_mutated(&self, attr: &Attr, mutation: AttributeMutation) {
        self.super_type().unwrap().attribute_mutated(attr, mutation);
        match (attr.local_name(), mutation) {
            (name, AttributeMutation::Set(_)) if name.starts_with("on") => {
                let evtarget = self.upcast::<EventTarget>();
                let source_line = 1; //TODO(#9604) get current JS execution line
                evtarget.set_event_handler_uncompiled(
                    window_from_node(self).get_url(),
                    source_line,
                    &name[2..],
                    // FIXME(ajeffrey): Convert directly from AttrValue to DOMString
                    DOMString::from(&**attr.value()),
                );
            },
            _ => {},
        }
    }

    fn parse_plain_attribute(&self, name: &LocalName, value: DOMString) -> AttrValue {
        match name {
            &local_name!("itemprop") => AttrValue::from_serialized_tokenlist(value.into()),
            &local_name!("itemtype") => AttrValue::from_serialized_tokenlist(value.into()),
            _ => self
                .super_type()
                .unwrap()
                .parse_plain_attribute(name, value),
        }
    }
}

impl Activatable for HTMLElement {
    fn as_element(&self) -> &Element {
        self.upcast::<Element>()
    }

    fn is_instance_activatable(&self) -> bool {
        self.as_element().local_name() == &local_name!("summary")
    }

    // Basically used to make the HTMLSummaryElement activatable (which has no IDL definition)
    fn activation_behavior(&self, _event: &Event, _target: &EventTarget) {
        self.summary_activation_behavior();
    }
}
