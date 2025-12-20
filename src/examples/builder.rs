use chk::*;

use crate::state::{CurrentProject, AllProjects};

pub struct RampBuilder;

impl Application for RampBuilder { // needs to be a fixed vector of 6 with minimum 1
    fn start(ctx: &mut Context) -> Vec<Root> {
        ctx.state().set(CurrentProject::default());
        ctx.state().set(AllProjects::default());
        vec![Root::new(RootContent::icon("home"), Home::build(ctx))]
    }

    fn theme(_ctx: &mut Assets) -> Theme { Theme::Dark(Color::from_hex("#eb343a", 255)) }

    fn on_event(ctx: &mut Context, event: Box<dyn Event>) -> Vec<Box<dyn Event>> {
        if event.downcast_ref::<TickEvent>().is_some() { // change to check for a Save(t) event where t contains the tag
            ctx.state().get_named::<String>("ProjectNameInput").cloned().and_then(|name| {
                ctx.state().get_mut::<CurrentProject>().map(|tx| tx.inner.name = name)
            });
        }

        vec![event]
    }
}

#[derive(Debug, Clone)]
pub struct Home;
impl Home {
    fn build(ctx: &mut Context) -> RootPage {
        let projects = ctx.state().get_or_default::<AllProjects>().inner.iter().map(|project| {
            ListItem::avatar(project.avatar.clone(), &project.name, &format!("Created {}", &project.date), None, &project.id)
        }).collect::<Vec<_>>();

        RootPage::new(
            "My projects",
            vec![
                Display::list(None, projects, Some(MyProjects::build()), Some("No projects yet.\nGet started by creating a new project."))
            ],
            None,
            RootBumper::new("New Project", CreateProject::build()),
            None,
        )
    }
    // fn on_event(&self, _ctx: &mut Context, event: Box<dyn Event>) -> Vec<Box<dyn Event>> {vec![event]}
}

// impl Home {
//     fn build(ctx: &mut Context) -> RootPage {
//         let projects = ctx.state().get_or_default::<AllProjects>().inner.iter().map(|project| {
//             ListItem::avatar(project.avatar.clone(), &project.name, &format!("Created {}", &project.date), None, &project.id)
//         }).collect::<Vec<_>>();

//         RootPage::new("My projects", 
//             vec![
//                 Display::list(None, projects, Some(MyProjects::build()), Some("No projects yet.\nGet started by creating a new project."))
//             ], 
//             None,
//             RootBumper::new("New Project", CreateProject::build()),
//             None
//         )
//     }
// }

pub struct MyProjects;
impl MyProjects {
    pub fn build() -> Flow {
        Flow::new(vec![
            // Box::new(|_state: &mut State| PageType::input("Orange", 
            //     Input::checklist(vec![
            //         ChecklistItem::new("Walk dog", "Due: 9/11/01", "WalkDogItem", false),
            //     ]),
            //     Bumper::default()
            // )),

            Box::new(|_state: &mut State| PageType::display("Orange", 
                vec![
                    Display::avatar(AvatarContent::icon("home", AvatarIconStyle::Brand)),
                    Display::list(Some("Release platforms"), vec![
                        ListItem::avatar(AvatarContent::icon("settings", AvatarIconStyle::Secondary), "iOS", "Default iOS Configuration", None, "ios_default_config"),
                        ListItem::avatar(AvatarContent::icon("settings", AvatarIconStyle::Secondary), "MacOS", "Default MacOS Configuration", None, "macos_default_config"),
                    ], Some(IOSSettings::build()), None),
                    Display::list(Some("Connected devices"), vec![
                        ListItem::avatar(AvatarContent::icon("settings", AvatarIconStyle::Secondary), "iOS", "USB Connected Device", None, "ios_usb_device"),
                        ListItem::avatar(AvatarContent::icon("settings", AvatarIconStyle::Secondary), "Android", "USB Connected Device", None, "android_usb_device"),
                    ], Some(Flow::default()), None),
                ], 
                Some(("settings".to_string(), EditProject::build())),
                Bumper::None, 
                Offset::Start
            ))
        ])
    }
}

pub struct EditProject;
impl EditProject {
    pub fn build() -> Flow {
        Flow::new(vec![
            Box::new(|_state: &mut State| PageType::settings("Project settings", 
                AvatarContent::icon("settings", AvatarIconStyle::Secondary), 
                vec![("Project name".to_string(), "ProjectNameInput".to_string(), Box::new(|ctx: &mut Context| ctx.state().get_mut::<CurrentProject>().map(|p| p.inner.name.is_empty()).unwrap_or_default()) as Box<dyn ValidityFn>)],
                Bumper::custom("Save", Action::None)
            ))
        ])
    }
}


pub struct CreateProject;
impl CreateProject {
    pub fn build() -> Flow {
        let project = Box::new(|_state: &mut State| PageType::settings("New project", 
            AvatarContent::icon("settings", AvatarIconStyle::Secondary), 
            vec![("Project name".to_string(), "ProjectNameInput".to_string(), Box::new(|ctx: &mut Context| ctx.state().get_mut::<CurrentProject>().map(|p| p.inner.name.is_empty()).unwrap_or_default())  as Box<dyn ValidityFn>)],
            Bumper::default()
        ));

        let success = |_state: &mut State| PageType::success("Project created", "checkmark", "Project 'orange' created");

        let on_submit = |ctx: &mut Context| println!("Creating project... {:?}", ctx.state().get::<CurrentProject>());
        Flow::form(vec![Box::new(project)], None, Box::new(success), on_submit)
    }
}

pub struct IOSSettings;
impl IOSSettings {
    pub fn build() -> Flow {
        let page = |_state: &mut State| PageType::input("iOS Settings", 
            Input::text("Bundle ID", None, "BundleIDInput", |_: &mut Context| {false}), 
            Bumper::double(
                "Debug", Action::custom(|_ctx: &mut Context| {println!("Debug bulid...")}), 
                "Release", Action::custom(|_ctx: &mut Context| {println!("Release bulid...")})
            )
        );

        Flow::new(vec![Box::new(page)])
    }
}